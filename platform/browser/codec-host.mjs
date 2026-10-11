// Minimal standard WebAssembly/WebCrypto binding. Rust owns codec/ID semantics.
export const MAX_WIRE_BYTES = 4 * 1024 * 1024;
export function codecHost(instance, cryptoProvider = globalThis.crypto) {
  const e = instance?.exports;
  if (!(e?.memory instanceof WebAssembly.Memory) ||
      ['eyl_allocate_input', 'eyl_process', 'eyl_output_ptr', 'eyl_output_len'].some(k => typeof e[k] !== 'function')) {
    throw new Error('Wasm codec interface unavailable');
  }
  function process(bytes, mode = 1) {
    if (!(bytes instanceof Uint8Array) || bytes.length === 0 || bytes.length > MAX_WIRE_BYTES ||
        !Number.isInteger(mode) || mode < 0 || mode > 3) throw new Error('Invalid bounded codec input');
    // Copy before allocation: caller input may alias Wasm memory or a Buffer.
    const input = new Uint8Array(bytes);
    const pointer = e.eyl_allocate_input(input.length);
    if (!pointer || pointer + input.length > e.memory.buffer.byteLength) throw new Error('Wasm input unavailable');
    new Uint8Array(e.memory.buffer, pointer, input.length).set(input);
    if (e.eyl_process(input.length, mode) !== 0) throw new Error('Invalid canonical codec data');
    const outputPointer = e.eyl_output_ptr(), length = e.eyl_output_len();
    if (!outputPointer || !length || length > MAX_WIRE_BYTES || outputPointer + length > e.memory.buffer.byteLength) {
      throw new Error('Wasm output unavailable');
    }
    // Copy before any await or new allocation; memory can grow on future calls.
    return new Uint8Array(e.memory.buffer, outputPointer, length).slice();
  }
  async function digest(bytes) {
    if (!(bytes instanceof Uint8Array) || bytes.length > MAX_WIRE_BYTES) throw new Error('Invalid digest input');
    if (typeof cryptoProvider?.subtle?.digest !== 'function') throw new Error('WebCrypto SHA-256 unavailable');
    return new Uint8Array(await cryptoProvider.subtle.digest('SHA-256', new Uint8Array(bytes)));
  }
  function randomId(kind) {
    if (!Number.isInteger(kind) || kind < 0 || kind > 6) throw new Error('Invalid ID kind');
    if (typeof cryptoProvider?.getRandomValues !== 'function') throw new Error('WebCrypto entropy unavailable');
    const input = new Uint8Array(17);
    input[0] = kind;
    cryptoProvider.getRandomValues(input.subarray(1));
    return new TextDecoder('utf-8', { fatal: true }).decode(process(input, 3));
  }
  return Object.freeze({ process, digest, randomId });
}
