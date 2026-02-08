// Hello World
//
// Prints a custom message to the viewport console.
// Set your message in the Inspector.

fn props() {
    #{
        message: #{ type: "string", value: "Hello from Rhai!", hint: "Message to print" },
    }
}

fn on_ready() {
    log(message);
}
