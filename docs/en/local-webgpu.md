# Local WebGPU scene

G20 adds a small browser WebGPU scene to the existing local task console. It is a view-only demonstration and has no connection to task execution, session authority or persisted task state.

Start the local service with the native CLI and open the printed `http://127.0.0.1:<port>/console` origin in a browser with WebGPU enabled. The page obtains a browser adapter and device, loads the owned WGSL shader from the same loopback origin, and draws three bounded cubes on a WebGPU canvas. Click a cube or focus the canvas and use the arrow keys to select and move it. Labeled buttons provide the same operations.

If `navigator.gpu` or an adapter is unavailable, the page shows `UNSUPPORTED` and disables scene controls. If the device's `lost` promise resolves, it shows `DEVICE_LOST`, disables controls and releases its buffers. It does not switch to CPU or WebGL rendering. Reloading requests a new device. Device loss in the deterministic fixture is simulated; it is not a forced hardware failure.

The scene is deliberately narrow. Its one-browser/M4 acceptance does not establish cross-browser support, WebGPU-to-Metal resource sharing, frame-time, energy, or model execution. Task console behavior remains independent of the scene.
