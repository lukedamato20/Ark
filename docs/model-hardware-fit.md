# Model hardware-fit method (`ark-fit-v1`)

Ark reports categories, never a numeric score:

- **Excellent:** currently available system memory is at least 3× the model's reported or reviewed approximate download size.
- **Good:** at least 2×.
- **Constrained:** at least 1.25×; runtime and context memory may leave little headroom.
- **Not recommended:** below 1.25×.
- **Unknown:** model size or memory evidence is missing, or the provider executes away from the local loopback device.

This is conservative guidance, not a speed claim. File size is not total runtime memory. `ark-fit-v1` does not claim accelerator backend, VRAM, offload, KV-cache, or throughput knowledge; those remain explicitly unknown until Ark has qualified cross-platform evidence. Hardware evidence stays local and is included in diagnostics only through the existing reviewed diagnostics workflow.

All file-size-to-memory categories carry **low confidence**, because download size is only a conservative proxy for runtime allocation. Unknown assessments carry **insufficient confidence**. Ark displays that confidence beside the category and exposes the concrete reason as text.
