use server::Server;

fn main() {
    let webserver = Server::new();
    webserver.run();
}
