#!/usr/bin/env python3
"""
FajarQuant — KV Cache Distribution Analysis

Analyzes the eigenvalue/singular value structure of extracted KV cache data.
Compares real data distribution with synthetic low-rank data used in benchmarks.

Usage:
    python scripts/analyze_kv_distribution.py --data data/kv_cache/
"""

import argparse
import json
import os
import sys

import numpy as np


def analyze_layer_head(keys_path: str, values_path: str) -> dict:
    """Analyze singular values for one layer-head pair."""
    keys = np.load(keys_path)   # (num_kv_heads, seq_len, d_head) or (seq_len, d_head)
    values = np.load(values_path)

    if keys.ndim == 3:
        # Multi-head: average over heads
        results = []
        for h in range(keys.shape[0]):
            k = keys[h]
            v = values[h]
            if k.shape[0] < 2:
                continue
            k_sv = np.linalg.svd(k, compute_uv=False)
            v_sv = np.linalg.svd(v, compute_uv=False)
            results.append({
                "key_sv": k_sv[:5].tolist(),
                "val_sv": v_sv[:5].tolist(),
                "key_rank_ratio": float(k_sv[0] / (k_sv[1] + 1e-10)),
                "val_rank_ratio": float(v_sv[0] / (v_sv[1] + 1e-10)),
                "key_energy_top3": float(np.sum(k_sv[:3]**2) / (np.sum(k_sv**2) + 1e-10)),
                "val_energy_top3": float(np.sum(v_sv[:3]**2) / (np.sum(v_sv**2) + 1e-10)),
            })
        return results
    else:
        k_sv = np.linalg.svd(keys, compute_uv=False)
        v_sv = np.linalg.svd(values, compute_uv=False)
        return [{
            "key_sv": k_sv[:5].tolist(),
            "val_sv": v_sv[:5].tolist(),
            "key_rank_ratio": float(k_sv[0] / (k_sv[1] + 1e-10)),
            "val_rank_ratio": float(v_sv[0] / (v_sv[1] + 1e-10)),
            "key_energy_top3": float(np.sum(k_sv[:3]**2) / (np.sum(k_sv**2) + 1e-10)),
            "val_energy_top3": float(np.sum(v_sv[:3]**2) / (np.sum(v_sv**2) + 1e-10)),
        }]


def generate_synthetic_comparison(d: int, seq_len: int, num_samples: int = 50) -> dict:
    """Generate synthetic low-rank data for comparison (same as FajarQuant benchmarks)."""
    key_ratios = []
    val_ratios = []
    key_energy = []

    rng = np.random.default_rng(42)
    for _ in range(num_samples):
        # Synthetic low-rank: rank-3 structure + noise
        base = rng.standard_normal((seq_len, 3)) @ rng.standard_normal((3, d))
        noise = rng.standard_normal((seq_len, d)) * 0.1
        synthetic = base + noise

        sv = np.linalg.svd(synthetic, compute_uv=False)
        key_ratios.append(sv[0] / (sv[1] + 1e-10))
        key_energy.append(np.sum(sv[:3]**2) / (np.sum(sv**2) + 1e-10))

    return {
        "rank_ratio_mean": float(np.mean(key_ratios)),
        "rank_ratio_std": float(np.std(key_ratios)),
        "energy_top3_mean": float(np.mean(key_energy)),
        "energy_top3_std": float(np.std(key_energy)),
    }


def main(data_dir: str):
    meta_path = os.path.join(data_dir, "metadata.json")
    if not os.path.exists(meta_path):
        print(f"Error: {meta_path} not found. Run extract_kv_cache.py first.")
        sys.exit(1)

    with open(meta_path) as f:
        meta = json.load(f)

    print(f"Model: {meta['model']}")
    print(f"Layers: {meta['num_layers']}, KV heads: {meta['num_kv_heads']}, d_head: {meta['d_head']}")
    print(f"Prompts: {meta['num_prompts']}")
    print()

    # Analyze first 10 prompts across all layers
    num_analyze = min(10, meta["num_prompts"])
    all_key_ratios = []
    all_val_ratios = []
    all_key_energy = []
    all_val_energy = []
    per_layer_key_ratio = {l: [] for l in range(meta["num_layers"])}

    for i in range(num_analyze):
        prompt_dir = os.path.join(data_dir, f"prompt_{i:03d}")
        if not os.path.exists(prompt_dir):
            continue

        for layer in range(meta["num_layers"]):
            kp = os.path.join(prompt_dir, f"layer_{layer:02d}_keys.npy")
            vp = os.path.join(prompt_dir, f"layer_{layer:02d}_values.npy")
            if not os.path.exists(kp):
                continue

            results = analyze_layer_head(kp, vp)
            for r in results:
                all_key_ratios.append(r["key_rank_ratio"])
                all_val_ratios.append(r["val_rank_ratio"])
                all_key_energy.append(r["key_energy_top3"])
                all_val_energy.append(r["val_energy_top3"])
                per_layer_key_ratio[layer].append(r["key_rank_ratio"])

    if not all_key_ratios:
        print("No data found to analyze.")
        return

    kr = np.array(all_key_ratios)
    vr = np.array(all_val_ratios)
    ke = np.array(all_key_energy)
    ve = np.array(all_val_energy)

    print("=" * 60)
    print("REAL KV CACHE DISTRIBUTION ANALYSIS")
    print("=" * 60)
    print(f"\nSamples analyzed: {len(kr)} (layer-head pairs)")
    print(f"\nKey SV1/SV2 ratio:   {kr.mean():.2f} +/- {kr.std():.2f}  (median: {np.median(kr):.2f})")
    print(f"Value SV1/SV2 ratio: {vr.mean():.2f} +/- {vr.std():.2f}  (median: {np.median(vr):.2f})")
    print(f"Key top-3 energy:    {ke.mean():.3f} +/- {ke.std():.3f}")
    print(f"Value top-3 energy:  {ve.mean():.3f} +/- {ve.std():.3f}")

    # Per-layer breakdown
    print(f"\nPer-layer key SV1/SV2 ratio:")
    for layer in sorted(per_layer_key_ratio.keys()):
        vals = per_layer_key_ratio[layer]
        if vals:
            print(f"  Layer {layer:2d}: {np.mean(vals):6.2f} +/- {np.std(vals):5.2f}")

    # Synthetic comparison
    print(f"\n{'=' * 60}")
    print("SYNTHETIC COMPARISON (rank-3 + noise)")
    print("=" * 60)
    syn = generate_synthetic_comparison(meta["d_head"], 128)
    print(f"Synthetic SV1/SV2 ratio:  {syn['rank_ratio_mean']:.2f} +/- {syn['rank_ratio_std']:.2f}")
    print(f"Synthetic top-3 energy:   {syn['energy_top3_mean']:.3f} +/- {syn['energy_top3_std']:.3f}")

    print(f"\n{'=' * 60}")
    print("COMPARISON")
    print("=" * 60)
    if kr.mean() > 3.0:
        print(f"Real data has STRONG low-rank structure (ratio={kr.mean():.1f} > 3)")
        print("=> FajarQuant adaptive PCA rotation should provide significant improvement")
    elif kr.mean() > 1.5:
        print(f"Real data has MODERATE low-rank structure (ratio={kr.mean():.1f})")
        print("=> FajarQuant should improve over random rotation")
    else:
        print(f"Real data has WEAK low-rank structure (ratio={kr.mean():.1f})")
        print("=> Random rotation may be sufficient")

    real_vs_syn = kr.mean() / syn["rank_ratio_mean"]
    print(f"\nReal/Synthetic ratio: {real_vs_syn:.2f}x")

    # Save analysis results
    analysis = {
        "real_key_ratio_mean": float(kr.mean()),
        "real_key_ratio_std": float(kr.std()),
        "real_val_ratio_mean": float(vr.mean()),
        "real_key_energy_mean": float(ke.mean()),
        "real_val_energy_mean": float(ve.mean()),
        "synthetic_ratio_mean": syn["rank_ratio_mean"],
        "synthetic_energy_mean": syn["energy_top3_mean"],
        "num_samples": len(kr),
    }
    out_path = os.path.join(data_dir, "analysis.json")
    with open(out_path, "w") as f:
        json.dump(analysis, f, indent=2)
    print(f"\nAnalysis saved to {out_path}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Analyze KV cache distribution")
    parser.add_argument("--data", default="data/kv_cache/", help="KV cache directory")
    args = parser.parse_args()
    main(args.data)
