#!/bin/bash
# q6a-train-deploy.sh — End-to-end ML pipeline for Dragon Q6A
#
# Usage: ./scripts/q6a-train-deploy.sh [train_script.fj]
#
# Steps:
#   1. Train model on host (or Q6A) using Fajar Lang
#   2. Export FJML (full precision) and FJMQ (INT8 quantized)
#   3. Deploy model files to Q6A /opt/fj/models/
#   4. Run inference on Q6A
#
# Requirements: fj binary, SSH access to Q6A (192.168.100.2)

set -euo pipefail

Q6A_HOST="${Q6A_HOST:-radxa@192.168.100.2}"
Q6A_MODEL_DIR="/opt/fj/models"
TRAIN_SCRIPT="${1:-examples/mnist_train_full.fj}"

echo "=== Fajar Lang → Q6A ML Pipeline ==="
echo "Train script: $TRAIN_SCRIPT"
echo "Q6A host:     $Q6A_HOST"
echo ""

# Step 1: Train
echo "[1/4] Training..."
fj run "$TRAIN_SCRIPT"
echo ""

# Step 2: Check exported models
echo "[2/4] Checking exported models..."
for f in model_mnist.fjml model_mnist.fjmq; do
    if [ -f "$f" ]; then
        echo "  $f: $(wc -c < "$f") bytes"
    else
        echo "  WARNING: $f not found"
    fi
done
echo ""

# Step 3: Deploy to Q6A
echo "[3/4] Deploying to $Q6A_HOST:$Q6A_MODEL_DIR..."
ssh "$Q6A_HOST" "sudo mkdir -p $Q6A_MODEL_DIR"
scp model_mnist.fjml model_mnist.fjmq "$Q6A_HOST:/tmp/"
ssh "$Q6A_HOST" "sudo cp /tmp/model_mnist.fjml /tmp/model_mnist.fjmq $Q6A_MODEL_DIR/"
echo "  Models deployed to $Q6A_MODEL_DIR"
echo ""

# Step 4: Run inference on Q6A
echo "[4/4] Running inference on Q6A..."
ssh "$Q6A_HOST" "fj run /home/radxa/fajar-lang/examples/q6a_npu_classify.fj"
echo ""

echo "=== Pipeline complete ==="
