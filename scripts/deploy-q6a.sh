#!/usr/bin/env bash
# Deploy Fajar Lang to Radxa Dragon Q6A
#
# Usage:
#   ./scripts/deploy-q6a.sh <ip>                     # deploy binary only
#   ./scripts/deploy-q6a.sh <ip> --examples           # deploy binary + examples
#   ./scripts/deploy-q6a.sh <ip> --run hello.fj       # deploy + run a .fj file
#
# Environment:
#   Q6A_USER  — SSH user (default: radxa)
#   Q6A_PORT  — SSH port (default: 22)

set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "Usage: $0 <ip> [--examples] [--run <file.fj>]"
    exit 1
fi

Q6A_IP="$1"
Q6A_USER="${Q6A_USER:-radxa}"
Q6A_PORT="${Q6A_PORT:-22}"
SSH_OPTS="-o StrictHostKeyChecking=no -p ${Q6A_PORT}"

TARGET="aarch64-unknown-linux-gnu"
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="${PROJECT_DIR}/target/${TARGET}/release/fj"

if [[ ! -f "${BINARY}" ]]; then
    echo "Binary not found. Building first..."
    "${PROJECT_DIR}/scripts/cross-build-q6a.sh"
fi

echo "=== Deploying to Dragon Q6A @ ${Q6A_IP} ==="

# Deploy binary
echo "Uploading fj binary..."
scp ${SSH_OPTS} "${BINARY}" "${Q6A_USER}@${Q6A_IP}:~/bin/fj"
ssh ${SSH_OPTS} "${Q6A_USER}@${Q6A_IP}" "chmod +x ~/bin/fj"

# Deploy examples if requested
if [[ "${2:-}" == "--examples" ]]; then
    echo "Uploading examples..."
    ssh ${SSH_OPTS} "${Q6A_USER}@${Q6A_IP}" "mkdir -p ~/fj-examples"
    scp ${SSH_OPTS} -r "${PROJECT_DIR}/examples/"*.fj "${Q6A_USER}@${Q6A_IP}:~/fj-examples/"
    echo "Examples deployed to ~/fj-examples/"
fi

# Run if requested
if [[ "${2:-}" == "--run" ]] && [[ -n "${3:-}" ]]; then
    FJ_FILE="$3"
    echo "Uploading ${FJ_FILE}..."
    scp ${SSH_OPTS} "${FJ_FILE}" "${Q6A_USER}@${Q6A_IP}:/tmp/run.fj"
    echo "Running on Q6A..."
    ssh ${SSH_OPTS} "${Q6A_USER}@${Q6A_IP}" "~/bin/fj run /tmp/run.fj"
fi

echo ""
echo "=== Deploy complete ==="
echo "SSH: ssh ${Q6A_USER}@${Q6A_IP}"
echo "Run: ~/bin/fj run <file.fj>"
