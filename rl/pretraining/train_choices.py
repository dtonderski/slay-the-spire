"""Small CPU behavior-cloning baseline for observed run-level item slates.

This is NOT a complete run actor or a checkpoint accepted by rl/train.py.
It ranks card/relic alternatives conditional on taking an item. Test evaluation
is opt-in; epochs are fixed, with no test-based checkpoint selection.
"""

import argparse
import json
import math
import random
from collections import Counter, defaultdict
from collections.abc import Callable
from pathlib import Path

import torch
from torch import nn

from .source import digest, write_json


def feature_counts(example: dict, history: bool) -> dict[str, float]:
    """Strict whitelist: never flatten provenance, label, future data or seed."""
    result = {"kind:" + example["kind"]: 1.0}
    if not history:
        return result
    context = example["context"]
    result["ascension:" + str(context["ascension"])] = 1.0
    if "floor" in context:
        result["floor_scaled"] = context["floor"] / 60
    if "boss_ordinal" in context:
        result["boss_ordinal:" + str(context["boss_ordinal"])] = 1.0
    for key in ("neow_bonus", "neow_cost"):
        if key in context:
            result[key + ":" + str(context[key])] = 1.0
    for group, values in context.get("history", {}).items():
        for identity, count in values.items():
            if type(count) is not int or count < 0:
                raise ValueError("history counts must be nonnegative integers")
            result[f"history:{group}:{identity}"] = math.log1p(count)
    return result


def vocabularies(examples: list[dict], history: bool) -> tuple[dict[str, int], dict[str, int]]:
    # All vocabularies fitted ONLY on training examples. Index zero is unknown.
    features = sorted({key for example in examples for key in feature_counts(example, history)})
    items = sorted({item for example in examples for item in example["candidates"]})
    return ({key: index + 1 for index, key in enumerate(features)}, {key: index + 1 for index, key in enumerate(items)})


class ChoiceModel(nn.Module):
    def __init__(self, features: dict[str, int], items: dict[str, int], *, width: int = 32, history: bool = True):
        super().__init__()
        self.features, self.items, self.history = features, items, history
        self.context_embedding = nn.Embedding(len(features) + 1, width)
        self.item_embedding = nn.Embedding(len(items) + 1, width)
        self.item_bias = nn.Embedding(len(items) + 1, 1)
        self.query = nn.Sequential(nn.Linear(width, width), nn.Tanh(), nn.Linear(width, width))
        nn.init.zeros_(self.item_bias.weight)
        # Unknown input/output identities remain a neutral, shared fallback.
        with torch.no_grad():
            self.context_embedding.weight[0].zero_()
            self.item_embedding.weight[0].zero_()
        self.context_embedding.padding_idx = 0
        self.item_embedding.padding_idx = 0
        self.item_bias.padding_idx = 0

    def forward(self, examples: list[dict]) -> tuple[torch.Tensor, torch.Tensor]:
        feature_rows = [feature_counts(e, self.history) for e in examples]
        nf = max(len(row) for row in feature_rows)
        na = max(len(e["candidates"]) for e in examples)
        feature_ids = torch.zeros((len(examples), nf), dtype=torch.long)
        weights = torch.zeros((len(examples), nf))
        item_ids = torch.zeros((len(examples), na), dtype=torch.long)
        valid = torch.zeros((len(examples), na), dtype=torch.bool)
        labels = []
        for i, (example, row) in enumerate(zip(examples, feature_rows, strict=True)):
            for j, (key, weight) in enumerate(sorted(row.items())):
                feature_ids[i, j] = self.features.get(key, 0)
                weights[i, j] = weight
            candidates = example["candidates"]
            item_ids[i, : len(candidates)] = torch.tensor([self.items.get(c, 0) for c in candidates])
            valid[i, : len(candidates)] = True
            labels.append(candidates.index(example["label"]))
        context = (self.context_embedding(feature_ids) * weights.unsqueeze(-1)).sum(dim=1)
        scores = (self.item_embedding(item_ids) * self.query(context).unsqueeze(1)).sum(dim=-1)
        scores = scores + self.item_bias(item_ids).squeeze(-1)
        return scores.masked_fill(~valid, float("-inf")), torch.tensor(labels)


def validate_examples(examples: list[dict]) -> None:
    groups: dict[str, str] = {}
    for e in examples:
        if e["schema"] != 1 or e["task"] != "conditional_on_taking_item" or e["kind"] not in ("card", "relic"):
            raise ValueError("unsupported training example; no hypothetical route data accepted")
        candidates = e["candidates"]
        if not isinstance(candidates, list) or len(candidates) < 2 or len(candidates) != len(set(candidates)):
            raise ValueError("expected distinct candidate equivalence classes")
        if e["label"] not in candidates or e["split"] not in ("train", "validation", "test"):
            raise ValueError("invalid label or split")
        if groups.setdefault(e["run_group"], e["split"]) != e["split"]:
            raise ValueError("run group crosses splits")


def load_dataset(path: Path) -> tuple[bytes, list[dict]]:
    summary_path = path.parent / "summary.json"
    if not summary_path.exists():
        raise ValueError("missing completed extraction summary")
    summary = json.loads(summary_path.read_text())
    raw = path.read_bytes()
    if summary.get("status") != "complete" or summary.get("schema") != 1:
        raise ValueError("incomplete or unsupported extraction")
    if summary.get("examples_sha256") != digest(raw):
        raise ValueError("dataset hash differs from completed extraction")
    examples = [json.loads(line) for line in raw.splitlines()]
    validate_examples(examples)
    return raw, examples


@torch.no_grad()
def evaluate(
    model: Callable[[list[dict]], tuple[torch.Tensor, torch.Tensor]],
    examples: list[dict],
    picked_counts: Counter,
    batch_size: int,
) -> dict:
    buckets = defaultdict(list)
    for start in range(0, len(examples), batch_size):
        batch = examples[start : start + batch_size]
        scores, targets = model(batch)
        nll = -scores.log_softmax(dim=1)[torch.arange(len(batch)), targets]
        for i, example in enumerate(batch):
            n = len(example["candidates"])
            logits = scores[i, :n]
            tied = logits == logits.max()
            accuracy = float(tied[targets[i]]) / int(tied.sum())
            frequencies = [picked_counts[c] + 1 for c in example["candidates"]]
            label_index = int(targets[i])
            record = {
                "nll": float(nll[i]),
                "accuracy": accuracy,
                "uniform_nll": math.log(n),
                "uniform_accuracy": 1 / n,
                "frequency_nll": -math.log(frequencies[label_index] / sum(frequencies)),
                "frequency_accuracy": (frequencies[label_index] == max(frequencies))
                / frequencies.count(max(frequencies)),
            }
            for key in ("all", example["kind"], f"a{example['context']['ascension']}"):
                buckets[key].append((example["run_group"], record))
    result = {}
    for name, entries in buckets.items():
        by_run = defaultdict(list)
        for group, record in entries:
            by_run[group].append(record["nll"])
        result[name] = {
            "decisions": len(entries),
            "runs": len(by_run),
            **{key: sum(row[key] for _, row in entries) / len(entries) for key in entries[0][1]},
            "run_mean_nll": sum(sum(v) / len(v) for v in by_run.values()) / len(by_run),
        }
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--context", choices=("slate", "history"), default="history")
    parser.add_argument("--epochs", type=int, default=5)
    parser.add_argument("--batch-size", type=int, default=128)
    parser.add_argument("--width", type=int, default=32)
    parser.add_argument("--lr", type=float, default=0.001)
    parser.add_argument("--seed", type=int, default=17)
    parser.add_argument("--evaluate-test", action="store_true")
    args = parser.parse_args()
    if min(args.epochs, args.batch_size, args.width) < 1 or not 0 < args.lr < 1:
        parser.error("positive epochs, batch size, width and lr in (0,1) required")
    raw, examples = load_dataset(args.dataset)
    groups = {split: [e for e in examples if e["split"] == split] for split in ("train", "validation", "test")}
    if not groups["train"] or not groups["validation"]:
        raise ValueError("nonempty training and validation sets required")
    args.output_dir.mkdir(parents=True, exist_ok=False)
    torch.set_num_threads(1)
    torch.manual_seed(args.seed)
    rng = random.Random(args.seed)
    features, items = vocabularies(groups["train"], args.context == "history")
    model = ChoiceModel(features, items, width=args.width, history=args.context == "history")
    optimizer = torch.optim.Adam(model.parameters(), lr=args.lr)
    counts = Counter(e["label"] for e in groups["train"])
    learning = []
    for epoch in range(args.epochs + 1):
        if epoch:
            model.train()
            indices = list(range(len(groups["train"])))
            rng.shuffle(indices)
            for start in range(0, len(indices), args.batch_size):
                batch = [groups["train"][i] for i in indices[start : start + args.batch_size]]
                optimizer.zero_grad()
                scores, targets = model(batch)
                loss = nn.functional.cross_entropy(scores, targets)
                if not torch.isfinite(loss):
                    raise ValueError("nonfinite supervised loss")
                loss.backward()
                optimizer.step()
        model.eval()
        metrics = evaluate(model, groups["validation"], counts, args.batch_size)
        learning.append({"epoch": epoch, "validation": metrics})
        print(json.dumps(learning[-1]), flush=True)
    config = {key: str(value) if isinstance(value, Path) else value for key, value in vars(args).items()}
    config["dataset_sha256"] = digest(raw)
    torch.save(
        {
            "format": "slaythedata_choice_bc_v1",
            "model": model.state_dict(),
            "config": config,
            "features": features,
            "items": items,
        },
        args.output_dir / "model.pt",
    )
    write_json(
        args.output_dir / "metrics.json",
        {
            "config": config,
            "learning": learning,
            "test": evaluate(model, groups["test"], counts, args.batch_size) if args.evaluate_test else None,
            "limitations": "Offline conditional item ranking only; not an RL warm-start or evidence of run strength.",
        },
    )


if __name__ == "__main__":
    main()
