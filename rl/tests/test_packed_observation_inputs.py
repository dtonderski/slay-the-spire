"""Packed inputs own their storage; only raw public arrays cross this boundary."""

import unittest

import numpy as np
import torch

from encoders.numeric import FeatureArrays, upload_features


class PackedObservationInputTests(unittest.TestCase):
    def test_shapes_casts_empty_and_strided_inputs(self) -> None:
        for device in ["cpu"] + (["cuda"] if torch.cuda.is_available() else []):
            for dtype in (torch.float32, torch.float64, torch.bfloat16):
                with self.subTest(device=device, dtype=dtype):
                    source = np.arange(24, dtype=np.int64).reshape(4, 6)
                    source.setflags(write=False)
                    groups = {
                        "first": FeatureArrays(
                            {"ids": source[:, 1]},
                            {"state": source[:, ::2], "empty": np.empty((0, 11))},
                            {"first": [2, 2]},
                        ),
                        "second": FeatureArrays(
                            {"empty": np.empty((0,), dtype=np.int64)},
                            {"state": np.array([[1 / 3, 16777217, -16777217]], dtype=np.float64)},
                            {"second": [1]},
                        ),
                    }
                    result = upload_features(groups, torch.empty(0, device=device, dtype=dtype))
                    for group, values in groups.items():
                        for arrays, expected_dtype in ((values.integers, torch.long), (values.floats, dtype)):
                            for key, array in arrays.items():
                                expected = torch.tensor(array, dtype=expected_dtype, device=device)
                                torch.testing.assert_close(result[group][key], expected, rtol=0, atol=0)
                                self.assertFalse(result[group][key].requires_grad)
                    self.assertEqual(
                        result["first"]["state"].untyped_storage().data_ptr(),
                        result["second"]["state"].untyped_storage().data_ptr(),
                    )
                    np.testing.assert_array_equal(source, np.arange(24).reshape(4, 6))
                    self.assertEqual(upload_features({}, torch.empty(0, device=device, dtype=dtype)), {})

    def test_staging_lifetime_and_source_mutation(self) -> None:
        for device in ["cpu"] + (["cuda"] if torch.cuda.is_available() else []):
            outputs = []
            reference = torch.empty(0, device=device)
            for value in range(32):
                floats = np.full((128, 17), value + 0.25)
                integers = np.full(128, value, dtype=np.int64)
                raw = FeatureArrays({"ids": integers}, {"state": floats}, {})
                outputs.append(upload_features({"group": raw}, reference)["group"])
                floats.fill(-100)
                integers.fill(-100)
            for value, tensors in enumerate(outputs):
                torch.testing.assert_close(tensors["state"], torch.full_like(tensors["state"], value + 0.25))
                torch.testing.assert_close(tensors["ids"], torch.full_like(tensors["ids"], value))


if __name__ == "__main__":
    unittest.main()
