let connections = {};

function send(client) {
    callDBus(
        "com._2e3s.Awatcher",
        "/com/_2e3s/Awatcher",
        "com._2e3s.Awatcher",
        "NotifyActiveWindow",
        "caption" in client ? client.caption : "",
        "resourceClass" in client ? String(client.resourceClass) : "",
        "resourceName" in client ? String(client.resourceName) : ""
    );
}

let handler = function(client){
    if (client === null) {
        return;
    }
    if (!(client.internalId in connections)) {
        connections[client.internalId] = true;
        client.captionChanged.connect(function() {
            if (client.active) {
                send(client);
            }
        });
    }

    send(client);
};

let activationEvent = workspace.windowActivated ? workspace.windowActivated : workspace.clientActivated;
if (workspace.windowActivated) {
    workspace.windowActivated.connect(handler);
} else {
    // KDE version < 6
    workspace.clientActivated.connect(handler);
}
