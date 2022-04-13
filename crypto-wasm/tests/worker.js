importScripts('./pkg/crypto_art.js');

async function init_worker() {
    await wasm_bindgen('./pkg/crypto_art.wasm');

    const { AtomicLock } = wasm_bindgen;

    // Create a new object of the `NumberEval` struct.
    var num_eval = AtomicLock.new();

    // Set callback to handle messages passed to the worker.
    self.onmessage = async event => {
        // By using methods of a struct as reaction to messages passed to the
        // worker, we can preserve our state between messages.
        var worker_result = num_eval.is_even(event.data);

        // Send response back to be handled by callback in main thread.
        self.postMessage(worker_result);
    };
};

init_worker();