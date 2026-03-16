#!/usr/bin/env bash
# export-qnn.sh — ONNX → QNN model export pipeline for Dragon Q6A
#
# Automates the full QNN SDK model conversion workflow:
#   1. Validate input ONNX model
#   2. Convert ONNX → QNN model (.cpp + .bin)
#   3. Quantize to INT8 with calibration data
#   4. Compile model library (.so)
#   5. Generate HTP context binary (.bin)
#   6. Deploy to Dragon Q6A via SCP
#
# Prerequisites:
#   - QNN SDK installed (qnn-onnx-converter, qnn-model-lib-generator, qnn-context-binary-generator)
#   - SSH access to Q6A: radxa@192.168.100.2
#   - Calibration data in --calib-dir (raw input tensors)
#
# Usage:
#   ./scripts/export-qnn.sh model.onnx
#   ./scripts/export-qnn.sh model.onnx --calib-dir ./calib_data
#   ./scripts/export-qnn.sh model.onnx --no-deploy
#   ./scripts/export-qnn.sh --help

set -euo pipefail

VERSION="1.0.0"
SCRIPT_NAME="$(basename "$0")"
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# Default configuration
Q6A_HOST="${Q6A_HOST:-radxa@192.168.100.2}"
Q6A_MODEL_DIR="${Q6A_MODEL_DIR:-/opt/fj/models}"
Q6A_PORT="${Q6A_PORT:-22}"
SSH_OPTS="-o StrictHostKeyChecking=no -o ConnectTimeout=10 -p ${Q6A_PORT}"
OUTPUT_DIR="${OUTPUT_DIR:-./qnn_output}"
CALIB_DIR=""
DEPLOY=true
QUANTIZE=true
HTP_ARCH="v68"
INPUT_LAYOUT="NHWC"

# Color output (if terminal supports it)
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
log_ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

usage() {
    cat <<EOF
${SCRIPT_NAME} v${VERSION} — ONNX → QNN model export pipeline for Dragon Q6A

USAGE:
    ${SCRIPT_NAME} <input.onnx> [OPTIONS]

ARGUMENTS:
    <input.onnx>          Path to input ONNX model file

OPTIONS:
    --calib-dir <dir>     Calibration data directory for INT8 quantization
                          (contains raw input tensors as .raw files)
    --output-dir <dir>    Output directory (default: ./qnn_output)
    --no-deploy           Skip SCP deployment to Q6A
    --no-quantize         Skip INT8 quantization (export FP32 model)
    --htp-arch <arch>     HTP architecture: v68 (default), v69, v73
    --input-layout <fmt>  Input layout: NHWC (default), NCHW
    --host <user@ip>      Q6A SSH host (default: radxa@192.168.100.2)
    --model-dir <dir>     Remote model directory (default: /opt/fj/models)
    --help                Show this help message

EXAMPLES:
    # Basic export with default settings
    ${SCRIPT_NAME} mobilenet_v2.onnx

    # Export with calibration data for accurate INT8 quantization
    ${SCRIPT_NAME} mobilenet_v2.onnx --calib-dir ./calibration_images

    # Export without deploying to device
    ${SCRIPT_NAME} yolov8n.onnx --no-deploy --output-dir ./exported

    # Export FP32 model (no quantization)
    ${SCRIPT_NAME} resnet18.onnx --no-quantize

PIPELINE:
    ONNX Model
      │
      ├─[1] qnn-onnx-converter ──→ QNN Model (.cpp + .bin)
      │
      ├─[2] qnn-onnx-converter --quantize ──→ INT8 Quantized Model
      │     (uses calibration data from --calib-dir)
      │
      ├─[3] qnn-model-lib-generator ──→ Model Library (.so)
      │
      ├─[4] qnn-context-binary-generator ──→ HTP Context Binary (.bin)
      │     (target: Hexagon 770 ${HTP_ARCH})
      │
      └─[5] scp ──→ Deploy to Q6A /opt/fj/models/

ENVIRONMENT:
    Q6A_HOST        SSH target (default: radxa@192.168.100.2)
    Q6A_MODEL_DIR   Remote model path (default: /opt/fj/models)
    Q6A_PORT        SSH port (default: 22)
    OUTPUT_DIR      Local output directory (default: ./qnn_output)
    QNN_SDK_ROOT    QNN SDK root (auto-detected if not set)

EOF
    exit 0
}

# --- Argument Parsing ---

ONNX_INPUT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --help|-h)
            usage
            ;;
        --calib-dir)
            CALIB_DIR="$2"
            shift 2
            ;;
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --no-deploy)
            DEPLOY=false
            shift
            ;;
        --no-quantize)
            QUANTIZE=false
            shift
            ;;
        --htp-arch)
            HTP_ARCH="$2"
            shift 2
            ;;
        --input-layout)
            INPUT_LAYOUT="$2"
            shift 2
            ;;
        --host)
            Q6A_HOST="$2"
            shift 2
            ;;
        --model-dir)
            Q6A_MODEL_DIR="$2"
            shift 2
            ;;
        -*)
            log_error "Unknown option: $1"
            echo "Run '${SCRIPT_NAME} --help' for usage."
            exit 1
            ;;
        *)
            if [[ -z "${ONNX_INPUT}" ]]; then
                ONNX_INPUT="$1"
            else
                log_error "Unexpected argument: $1"
                exit 1
            fi
            shift
            ;;
    esac
done

if [[ -z "${ONNX_INPUT}" ]]; then
    log_error "No input ONNX model specified."
    echo "Run '${SCRIPT_NAME} --help' for usage."
    exit 1
fi

# --- Pre-flight Checks ---

echo "=== Fajar Lang QNN Export Pipeline v${VERSION} ==="
echo ""

# Check input file exists
if [[ ! -f "${ONNX_INPUT}" ]]; then
    log_error "ONNX model not found: ${ONNX_INPUT}"
    exit 1
fi

MODEL_NAME="$(basename "${ONNX_INPUT}" .onnx)"
log_info "Model: ${MODEL_NAME} (${ONNX_INPUT})"
log_info "Output: ${OUTPUT_DIR}/"
log_info "HTP arch: ${HTP_ARCH}"
log_info "Quantize: ${QUANTIZE}"
log_info "Deploy: ${DEPLOY}"
echo ""

# Check required tools
log_info "Checking QNN SDK tools..."
MISSING_TOOLS=0

check_tool() {
    if command -v "$1" &>/dev/null; then
        log_ok "$1 found: $(command -v "$1")"
    else
        log_error "$1 not found in PATH"
        MISSING_TOOLS=$((MISSING_TOOLS + 1))
    fi
}

check_tool "qnn-onnx-converter"
check_tool "qnn-model-lib-generator"
check_tool "qnn-context-binary-generator"

if [[ ${MISSING_TOOLS} -gt 0 ]]; then
    echo ""
    log_error "${MISSING_TOOLS} required tool(s) missing."
    echo ""
    echo "Install QNN SDK:"
    echo "  sudo apt install qnn-tools libqnn-dev libqnn1"
    echo ""
    echo "Or set QNN_SDK_ROOT and add to PATH:"
    echo "  export QNN_SDK_ROOT=/opt/qcom/aistack/qnn/2.40.0"
    echo "  export PATH=\$QNN_SDK_ROOT/bin/x86_64-linux-clang:\$PATH"
    exit 1
fi

# Check calibration data if quantizing
if [[ "${QUANTIZE}" == "true" ]] && [[ -n "${CALIB_DIR}" ]]; then
    if [[ ! -d "${CALIB_DIR}" ]]; then
        log_error "Calibration directory not found: ${CALIB_DIR}"
        exit 1
    fi
    CALIB_COUNT=$(find "${CALIB_DIR}" -name "*.raw" -type f | wc -l)
    if [[ ${CALIB_COUNT} -eq 0 ]]; then
        log_warn "No .raw files found in ${CALIB_DIR}. Quantization may use default ranges."
    else
        log_info "Calibration data: ${CALIB_COUNT} samples in ${CALIB_DIR}/"
    fi
fi

echo ""

# Create output directory
mkdir -p "${OUTPUT_DIR}"

# --- Step 1: Convert ONNX → QNN Model ---

echo "=== [1/5] ONNX → QNN Conversion ==="
log_info "Converting ${ONNX_INPUT} → QNN model..."

QNN_MODEL_CPP="${OUTPUT_DIR}/${MODEL_NAME}.cpp"
QNN_MODEL_BIN="${OUTPUT_DIR}/${MODEL_NAME}.bin"

CONVERTER_ARGS=(
    --input_network "${ONNX_INPUT}"
    --output_path "${OUTPUT_DIR}/${MODEL_NAME}"
    --input_layout "${INPUT_LAYOUT}"
)

qnn-onnx-converter "${CONVERTER_ARGS[@]}"

if [[ -f "${QNN_MODEL_CPP}" ]]; then
    log_ok "QNN model generated: ${QNN_MODEL_CPP}"
else
    log_error "Conversion failed — no output generated."
    exit 1
fi
echo ""

# --- Step 2: Quantize to INT8 ---

QNN_QUANT_CPP="${OUTPUT_DIR}/${MODEL_NAME}_quantized.cpp"
QNN_QUANT_BIN="${OUTPUT_DIR}/${MODEL_NAME}_quantized.bin"

if [[ "${QUANTIZE}" == "true" ]]; then
    echo "=== [2/5] INT8 Quantization ==="

    QUANT_ARGS=(
        --input_network "${ONNX_INPUT}"
        --output_path "${OUTPUT_DIR}/${MODEL_NAME}_quantized"
        --input_layout "${INPUT_LAYOUT}"
        --act_bitwidth 8
        --weight_bitwidth 8
        --bias_bitwidth 32
    )

    if [[ -n "${CALIB_DIR}" ]]; then
        # Build input list file from calibration data
        INPUT_LIST="${OUTPUT_DIR}/calib_input_list.txt"
        find "${CALIB_DIR}" -name "*.raw" -type f | sort > "${INPUT_LIST}"
        QUANT_ARGS+=(--input_list "${INPUT_LIST}")
        log_info "Using calibration data: ${CALIB_DIR}/ ($(wc -l < "${INPUT_LIST}") samples)"
    else
        log_warn "No calibration data — using default quantization ranges."
        log_warn "For best accuracy, provide --calib-dir with representative inputs."
    fi

    qnn-onnx-converter "${QUANT_ARGS[@]}"

    if [[ -f "${QNN_QUANT_CPP}" ]]; then
        log_ok "Quantized model: ${QNN_QUANT_CPP}"
    else
        log_error "Quantization failed."
        exit 1
    fi
else
    echo "=== [2/5] Quantization — SKIPPED ==="
    log_info "Using FP32 model (--no-quantize)."
    QNN_QUANT_CPP="${QNN_MODEL_CPP}"
    QNN_QUANT_BIN="${QNN_MODEL_BIN}"
fi
echo ""

# --- Step 3: Compile Model Library (.so) ---

echo "=== [3/5] Model Library Generation ==="

# Determine which model to compile
if [[ "${QUANTIZE}" == "true" ]]; then
    LIB_INPUT="${QNN_QUANT_CPP}"
    LIB_NAME="${MODEL_NAME}_quantized"
else
    LIB_INPUT="${QNN_MODEL_CPP}"
    LIB_NAME="${MODEL_NAME}"
fi

MODEL_LIB="${OUTPUT_DIR}/lib${LIB_NAME}.so"

LIB_ARGS=(
    -c "${LIB_INPUT}"
    -b "${OUTPUT_DIR}/${LIB_NAME}.bin"
    -o "${OUTPUT_DIR}"
    -t aarch64-android
)

log_info "Generating model library..."
qnn-model-lib-generator "${LIB_ARGS[@]}"

if [[ -f "${MODEL_LIB}" ]]; then
    LIB_SIZE=$(du -h "${MODEL_LIB}" | cut -f1)
    log_ok "Model library: ${MODEL_LIB} (${LIB_SIZE})"
else
    # Try alternate path (some SDK versions use different naming)
    ALT_LIB="${OUTPUT_DIR}/aarch64-android/lib${LIB_NAME}.so"
    if [[ -f "${ALT_LIB}" ]]; then
        mv "${ALT_LIB}" "${MODEL_LIB}"
        LIB_SIZE=$(du -h "${MODEL_LIB}" | cut -f1)
        log_ok "Model library: ${MODEL_LIB} (${LIB_SIZE})"
    else
        log_error "Library generation failed."
        exit 1
    fi
fi
echo ""

# --- Step 4: Generate HTP Context Binary ---

echo "=== [4/5] HTP Context Binary Generation ==="

CONTEXT_BIN="${OUTPUT_DIR}/${LIB_NAME}_htp_${HTP_ARCH}.bin"

# Map architecture to backend library
case "${HTP_ARCH}" in
    v68) HTP_SKEL="libQnnHtpV68Skel.so" ;;
    v69) HTP_SKEL="libQnnHtpV69Skel.so" ;;
    v73) HTP_SKEL="libQnnHtpV73Skel.so" ;;
    *)
        log_error "Unknown HTP architecture: ${HTP_ARCH}"
        exit 1
        ;;
esac

CONTEXT_ARGS=(
    --model "${MODEL_LIB}"
    --backend libQnnHtp.so
    --output_dir "${OUTPUT_DIR}"
    --binary_file "${LIB_NAME}_htp_${HTP_ARCH}.bin"
)

log_info "Generating HTP context binary (${HTP_ARCH})..."
log_info "Backend: libQnnHtp.so → ${HTP_SKEL}"

qnn-context-binary-generator "${CONTEXT_ARGS[@]}"

if [[ -f "${CONTEXT_BIN}" ]]; then
    CTX_SIZE=$(du -h "${CONTEXT_BIN}" | cut -f1)
    log_ok "Context binary: ${CONTEXT_BIN} (${CTX_SIZE})"
else
    log_error "Context binary generation failed."
    log_warn "This step requires the HTP backend libraries."
    log_warn "On x86_64 host, you may need to run this on the Q6A device."
    exit 1
fi
echo ""

# --- Step 5: Deploy to Q6A ---

if [[ "${DEPLOY}" == "true" ]]; then
    echo "=== [5/5] Deploy to Dragon Q6A ==="
    log_info "Target: ${Q6A_HOST}:${Q6A_MODEL_DIR}/"

    # Test SSH connectivity
    if ! ssh ${SSH_OPTS} "${Q6A_HOST}" "echo ok" &>/dev/null; then
        log_error "Cannot connect to ${Q6A_HOST}."
        log_warn "Skipping deployment. Files are in ${OUTPUT_DIR}/"
        log_warn "Manual deploy: scp ${CONTEXT_BIN} ${Q6A_HOST}:${Q6A_MODEL_DIR}/"
    else
        # Create remote directory
        ssh ${SSH_OPTS} "${Q6A_HOST}" "sudo mkdir -p ${Q6A_MODEL_DIR}"

        # Deploy context binary (primary artifact)
        log_info "Uploading context binary..."
        scp ${SSH_OPTS} "${CONTEXT_BIN}" "${Q6A_HOST}:/tmp/${LIB_NAME}_htp_${HTP_ARCH}.bin"
        ssh ${SSH_OPTS} "${Q6A_HOST}" "sudo cp /tmp/${LIB_NAME}_htp_${HTP_ARCH}.bin ${Q6A_MODEL_DIR}/"
        log_ok "Deployed: ${Q6A_MODEL_DIR}/${LIB_NAME}_htp_${HTP_ARCH}.bin"

        # Deploy model library
        log_info "Uploading model library..."
        scp ${SSH_OPTS} "${MODEL_LIB}" "${Q6A_HOST}:/tmp/lib${LIB_NAME}.so"
        ssh ${SSH_OPTS} "${Q6A_HOST}" "sudo cp /tmp/lib${LIB_NAME}.so ${Q6A_MODEL_DIR}/"
        log_ok "Deployed: ${Q6A_MODEL_DIR}/lib${LIB_NAME}.so"

        # Verify deployment
        echo ""
        log_info "Verifying deployment..."
        ssh ${SSH_OPTS} "${Q6A_HOST}" "ls -lh ${Q6A_MODEL_DIR}/${LIB_NAME}*"
        log_ok "Deployment verified."
    fi
else
    echo "=== [5/5] Deploy — SKIPPED ==="
    log_info "Files are in ${OUTPUT_DIR}/"
fi

echo ""

# --- Summary ---

echo "=== Export Summary ==="
echo ""
echo "  Model:            ${MODEL_NAME}"
echo "  Input:            ${ONNX_INPUT}"
echo "  QNN model:        ${QNN_MODEL_CPP}"
if [[ "${QUANTIZE}" == "true" ]]; then
    echo "  Quantized:        ${QNN_QUANT_CPP} (INT8)"
fi
echo "  Library:          ${MODEL_LIB}"
echo "  Context binary:   ${CONTEXT_BIN}"
echo "  HTP arch:         Hexagon 770 ${HTP_ARCH}"
echo ""

if [[ "${DEPLOY}" == "true" ]]; then
    echo "  Deployed to:      ${Q6A_HOST}:${Q6A_MODEL_DIR}/"
    echo ""
    echo "  Run inference on Q6A:"
    echo "    ssh ${Q6A_HOST}"
    echo "    fj run examples/q6a_npu_classify.fj"
else
    echo "  Deploy manually:"
    echo "    scp ${CONTEXT_BIN} ${Q6A_HOST}:${Q6A_MODEL_DIR}/"
fi

echo ""
echo "=== QNN export complete ==="
