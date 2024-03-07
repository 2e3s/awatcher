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

workspace.windowActivated.connect(function(client){
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
});
