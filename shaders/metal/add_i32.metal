#include <metal_stdlib>
using namespace metal;

kernel void add_i32(
    device const int* lhs [[buffer(0)]],
    device const int* rhs [[buffer(1)]],
    device int* output [[buffer(2)]],
    uint index [[thread_position_in_grid]]) {
    output[index] = lhs[index] + rhs[index];
}
