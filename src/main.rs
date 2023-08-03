use ulrichrs::Server;

fn main() {
    let mut webserver = Server::new();
    webserver.get("hello");
    webserver.get("");
    webserver.post("post");
    webserver.run();
}
