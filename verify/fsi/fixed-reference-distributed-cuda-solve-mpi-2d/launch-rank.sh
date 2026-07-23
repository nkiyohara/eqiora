#!/bin/sh
set -eu

# The registered one-host evidence requires an explicit, ordered list of
# physical CUDA selectors. Each MPI process receives exactly one selector;
# CUDA remaps that process-local device to ordinal zero.
selectors=${EQIORA_MPI_CUDA_DEVICE_SELECTORS:?set EQIORA_MPI_CUDA_DEVICE_SELECTORS to four distinct CUDA_VISIBLE_DEVICES selectors}

if [ -n "${OMPI_COMM_WORLD_LOCAL_RANK-}" ]; then
    local_rank=$OMPI_COMM_WORLD_LOCAL_RANK
elif [ -n "${MPI_LOCALRANKID-}" ]; then
    local_rank=$MPI_LOCALRANKID
elif [ -n "${MV2_COMM_WORLD_LOCAL_RANK-}" ]; then
    local_rank=$MV2_COMM_WORLD_LOCAL_RANK
elif [ -n "${SLURM_LOCALID-}" ]; then
    local_rank=$SLURM_LOCALID
else
    echo "cannot determine the MPI local rank for CUDA isolation" >&2
    exit 64
fi

case $local_rank in
    ''|*[!0-9]*)
        echo "MPI local rank is not a nonnegative integer: $local_rank" >&2
        exit 64
        ;;
esac

select_for_rank() {
    selector_list=$1
    requested_rank=$2
    set -f
    old_ifs=$IFS
    IFS=,
    set -- $selector_list
    IFS=$old_ifs

    index=0
    selected=
    for selector do
        if [ -z "$selector" ]; then
            echo "CUDA selector list contains an empty entry" >&2
            return 64
        fi
        if [ "$index" -eq "$requested_rank" ]; then
            selected=$selector
        fi
        index=$((index + 1))
    done
    printf '%s\n' "$selected"
}

selected=$(select_for_rank "$selectors" "$local_rank")

if [ -z "$selected" ]; then
    echo "CUDA selector list has no entry for MPI local rank $local_rank" >&2
    exit 64
fi

CUDA_VISIBLE_DEVICES=$selected
EQIORA_CUDA_DEVICE=0
export CUDA_VISIBLE_DEVICES EQIORA_CUDA_DEVICE
exec "$@"
