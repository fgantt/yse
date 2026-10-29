"""Packed records (`nnue_pack pack`) -> network inputs, on any torch device.

A record is 100 bytes: board[81] (0 empty, else 1 + 14*colour + piece type),
hand[14] (Black's P L N S G B R, then White's), side to move, teacher eval i16
(side to move's view), outcome i8 (+1 Black won), pad. Feature indices come from
the engine's own lookup tables (`nnue_pack tables`), so the trainer and the engine
cannot disagree about what a feature is; `verify.py` checks it.
"""
import json

import numpy as np
import torch

RECORD = 100
DTYPE = np.dtype([("board", "u1", 81), ("hand", "u1", 14), ("stm", "u1"),
                  ("eval", "<i2"), ("outcome", "i1"), ("pad", "u1")])
assert DTYPE.itemsize == RECORD


def open_records(path):
    return np.memmap(path, dtype=DTYPE, mode="r")


class Features:
    """Builds, per perspective, the input-row indices of a batch (padded with
    `pad`) and the matching factoriser rows (the index without its king bucket)."""

    def __init__(self, tables_path, device):
        t = json.load(open(tables_path))
        self.rows, self.stride = t["rows"], t["stride"]
        self.pad, self.shared_pad = self.rows, self.stride
        piece = torch.tensor(t["piece"], dtype=torch.int64)          # [frame, code, sq]
        self.piece = piece.to(device)
        self.king = torch.tensor(t["king"], dtype=torch.int64, device=device)  # [frame, sq]
        hand = torch.tensor(t["hand"], dtype=torch.int64)             # [frame, holder, type, k]
        # One slot per (holder, type, level): 2 x 38 per frame.
        slots = [(c, ty, k) for c in range(2) for ty, m in enumerate(t["max_hand"])
                 for k in range(1, m + 1)]
        self.slot_col = torch.tensor([c * 7 + ty for c, ty, _ in slots], device=device)
        self.slot_k = torch.tensor([k for _, _, k in slots], device=device)
        self.hand_idx = torch.stack([torch.tensor([hand[f, c, ty, k] for c, ty, k in slots])
                                     for f in range(2)]).to(device)   # [frame, 76]
        self.material_psqt = torch.tensor(t["material_psqt"], dtype=torch.float32, device=device)
        self.device = device

    def batch(self, rec):
        """`rec`: numpy structured array. Returns (stm idx, stm shared, nstm idx,
        nstm shared, eval, outcome to move), indices [B, 157] int64."""
        dev = self.device
        board = torch.from_numpy(np.ascontiguousarray(rec["board"])).to(dev).long()
        hand = torch.from_numpy(np.ascontiguousarray(rec["hand"])).to(dev).long()
        stm = torch.from_numpy(np.ascontiguousarray(rec["stm"])).to(dev).long()
        ev = torch.from_numpy(rec["eval"].astype(np.float32)).to(dev)
        outcome = torch.from_numpy(rec["outcome"].astype(np.float32)).to(dev)
        outcome = torch.where(stm == 0, outcome, -outcome)
        sq = torch.arange(81, device=dev)
        # Each side's king square: code 8 (Black king), 22 (White king).
        ksq = torch.stack([(board == 8).long().argmax(1), (board == 22).long().argmax(1)], 1)
        hand_on = hand[:, self.slot_col] >= self.slot_k               # [B, 76]
        out = []
        for frame in (stm, 1 - stm):
            base_board = self.piece[frame[:, None], board, sq[None, :]]   # [B, 81], -1 empty
            base_hand = self.hand_idx[frame]                              # [B, 76]
            base = torch.cat([base_board, base_hand], 1)
            valid = torch.cat([board > 0, hand_on], 1) & (base >= 0)
            off = self.king[frame, ksq.gather(1, frame[:, None]).squeeze(1)]
            out.append(torch.where(valid, base + off[:, None], self.pad))
            out.append(torch.where(valid, base, self.shared_pad))
        return (*out, ev, outcome)
