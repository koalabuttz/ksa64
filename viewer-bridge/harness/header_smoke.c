#include "../ksa64_viewer_bridge.h"

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(Ksa64ViewerAbiInfo) == 132, "ABI info layout drift");
_Static_assert(sizeof(Ksa64ViewerSpan) == 24, "span layout drift");
_Static_assert(sizeof(Ksa64ViewerOwnedBuffer) == 32, "buffer layout drift");
_Static_assert(sizeof(Ksa64ViewerSnapshot) == 184, "snapshot layout drift");
_Static_assert(sizeof(Ksa64ViewerStartRequestV1) == 48, "start request layout drift");
_Static_assert(sizeof(Ksa64ViewerOperationalViewV1) == 208, "operational view layout drift");
#endif

int main(void) {
    Ksa64ViewerAbiInfo info = {0};
    info.abi_version = KSA64_VIEWER_ABI_VERSION;
    info.struct_size = (uint32_t)sizeof(info);
    return info.struct_size == 132u ? 0 : 1;
}
