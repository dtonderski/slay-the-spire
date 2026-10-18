"""Supervise the item head actually used by train.py --task run --encoder choice.

Conditional item ranking only: no fabricated observations, historical deck,
Skip/Bowl targets, elite slates, route labels or run-value targets.
"""

import argparse
import json
import random
import time
from collections import Counter
from pathlib import Path

import torch
from run_training.item_prior import (
    ITEM_PRIOR_FORMAT,
    ITEM_PRIOR_PROTOCOL,
    ITEM_PRIOR_WIDTH,
    ItemPrior,
    ItemSlate,
)

from .source import digest, write_json
from .train_choices import evaluate, load_dataset


def slate_from_example(example: dict) -> ItemSlate:
    # Deliberately no flattening of history/provenance/outcomes or final inventories.
    if example["task"] != "conditional_on_taking_item" or example["kind"] not in (
        "card",
        "relic",
    ):
        raise ValueError(
            "Only observed conditional card/boss-relic choices are supported"
        )
    context = example["context"]
    ascension = context["ascension"]
    floor = context["floor"] if example["kind"] == "card" else None
    if type(ascension) is not int or not 0 <= ascension <= 20:
        raise ValueError("Invalid public ascension")
    if example["kind"] == "card" and (type(floor) is not int or not 0 <= floor <= 60):
        raise ValueError("Card floor must be known")
    return ItemSlate(example["kind"], ascension, floor, tuple(example["candidates"]))


class TrainingAdapter:
    def __init__(self, head: ItemPrior) -> None:
        self.head = head

    def __call__(self, examples: list[dict]) -> tuple[torch.Tensor, torch.Tensor]:
        scores = self.head([slate_from_example(e) for e in examples])
        labels = torch.tensor(
            [e["candidates"].index(e["label"]) for e in examples], device=scores.device
        )
        return scores, labels


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--epochs", type=int, default=5)
    parser.add_argument("--batch-size", type=int, default=128)
    parser.add_argument("--lr", type=float, default=0.001)
    parser.add_argument("--seed", type=int, default=17)
    parser.add_argument("--evaluate-test", action="store_true")
    args = parser.parse_args()
    if min(args.epochs, args.batch_size) < 1 or not 0 < args.lr < 1:
        parser.error("Positive counts and lr in (0,1) required")
    raw, examples = load_dataset(args.dataset)
    groups = {
        split: [e for e in examples if e["split"] == split]
        for split in ("train", "validation", "test")
    }
    if not groups["train"] or not groups["validation"]:
        raise ValueError("Nonempty training and validation required")
    args.output_dir.mkdir(parents=True, exist_ok=False)
    torch.set_num_threads(1)
    torch.manual_seed(args.seed)
    rng = random.Random(args.seed)
    items = tuple(sorted({item for e in groups["train"] for item in e["candidates"]}))
    head = ItemPrior(items)
    model = TrainingAdapter(head)
    optimizer = torch.optim.Adam(head.parameters(), lr=args.lr)
    counts = Counter(e["label"] for e in groups["train"])
    learning = []
    started = time.monotonic()
    for epoch in range(args.epochs + 1):
        if epoch:
            indices = list(range(len(groups["train"])))
            rng.shuffle(indices)
            for start in range(0, len(indices), args.batch_size):
                batch = [
                    groups["train"][i] for i in indices[start : start + args.batch_size]
                ]
                optimizer.zero_grad(set_to_none=True)
                scores, targets = model(batch)
                loss = torch.nn.functional.cross_entropy(scores, targets)
                if not torch.isfinite(loss):
                    raise ValueError("Nonfinite supervised loss")
                loss.backward()
                torch.nn.utils.clip_grad_norm_(
                    head.parameters(), 1.0, error_if_nonfinite=True
                )
                optimizer.step()
        metrics = evaluate(model, groups["validation"], counts, args.batch_size)
        learning.append({"epoch": epoch, "validation": metrics})
        print(json.dumps(learning[-1]), flush=True)
    config = {
        key: str(value) if isinstance(value, Path) else value
        for key, value in vars(args).items()
    }
    config.update(
        protocol=ITEM_PRIOR_PROTOCOL,
        width=ITEM_PRIOR_WIDTH,
        items=list(items),
        dataset_sha256=digest(raw),
        training_source_sha256=digest(Path(__file__).read_bytes()),
        head_source_sha256=digest(
            (Path(__file__).parents[1] / "run_training/item_prior.py").read_bytes()
        ),
    )
    torch.save(
        {"format": ITEM_PRIOR_FORMAT, "config": config, "model": head.state_dict()},
        args.output_dir / "model.pt",
    )
    write_json(
        args.output_dir / "metrics.json",
        {
            "config": config,
            "seconds": time.monotonic() - started,
            "learning": learning,
            "test": evaluate(model, groups["test"], counts, args.batch_size)
            if args.evaluate_test
            else None,
            "validation_supported_slates": sum(
                head.supported(slate_from_example(e)) for e in groups["validation"]
            ),
            "limitations": "Shared run-policy head only; conditional preferences do not supervise taking vs skipping. "
            "Full-context policy and critic stay fresh. No online strength claim from offline agreement.",
        },
    )


if __name__ == "__main__":
    main()
