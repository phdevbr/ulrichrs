use std::{
    fs,
    io::prelude::*,
    net::TcpListener,
    net::TcpStream,
    sync::{
        mpsc::{self},
        Arc, Mutex,
    },
    thread,
    // time::Duration,
};

enum Method {
    GET,
    POST,
}

struct Route<'a> {
    method: Method,
    path: &'a str,
}

pub struct Server<'a> {
    routes: Vec<Route<'a>>,
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);
        let mut workers = Vec::with_capacity(size);

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();

            match message {
                Ok(job) => {
                    println!("Worker {id} got a job; executing.");

                    job();
                }
                Err(_) => {
                    println!("Worker {id} disconnected; shutting down.");
                    break;
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}

impl<'a> Route<'a> {
    fn new(method: Method, path: &str) -> Route {
        Route { method, path }
    }
}

impl<'a> Server<'a> {
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    pub fn get(&mut self, path: &'a str) {
        self.routes.push(Route::new(Method::GET, path))
    }
    pub fn post(&mut self, path: &'a str) {
        self.routes.push(Route::new(Method::POST, path))
    }
    fn process_routes(&self) -> Vec<Vec<u8>> {
        let mut routes = Vec::<Vec<u8>>::new();
        for i in 0..self.routes.len() {
            let route = &self.routes[i];
            println!("{}", route.path);
            match route.method {
                Method::GET => {
                    let raw_path = format!("GET /{} HTTP/1.1\r\n", route.path).into_bytes();
                    // let raw = b"GET / HTTP/1.1\r\n";
                    println!("raw: {:?}", raw_path);
                    //println!("{:?}", raw_path);
                    routes.push(raw_path);
                }
                Method::POST => {
                    routes.push(format!("POST {} HTTP/1.1\r\n", route.path).into_bytes())
                }
            }
        }
        routes
    }
    pub fn run(&self, port: u16) {
        let port_addr = if !port.to_string().is_empty() {
            port.to_owned()
        } else {
            8080
        };
        let routes = Arc::new(Mutex::new(self.process_routes()));
        let listener = TcpListener::bind(format!("localhost:{}", port_addr)).unwrap();
        let pool = ThreadPool::new(8);
        println!("Server is running at localhost:{}", port_addr);
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            let routes_arc = Arc::clone(&routes);

            pool.execute(move || {
                handle_connection(stream, routes_arc);
            });
        }
    }
}

fn handle_connection(mut stream: TcpStream, routes_arc: Arc<Mutex<Vec<Vec<u8>>>>) {
    let routes = routes_arc.lock().unwrap();
    let mut buffer = [0; 1024].to_vec();
    stream.read(&mut buffer).unwrap();

    // let get: &[u8; 16] = b"GET / HTTP/1.1\r\n";
    // let sleep = b"GET /sleep HTTP/1.1\r\n";

    let (mut status_line, mut filename) = ("", "");

    for i in 0..routes.len() {
        println!("processing: {:?}", routes[i].to_owned());
        (status_line, filename) = if buffer.starts_with(&routes[i]) {
            ("HTTP/1.1 200 OK", "hello.html")
        } else {
            ("HTTP/1.1 404 NOT FOUND", "404.html")
        };
        // else if buffer.starts_with(sleep) {
        //     thread::sleep(Duration::from_secs(5));
        //     ("HTTP/1.1 200 OK", "hello.html")
        // }
    }

    let contents = fs::read_to_string(filename).unwrap();

    let response = format!(
        "{}\r\nContent-Length: {}\r\n\r\n{}",
        status_line,
        contents.len(),
        contents
    );

    stream.write_all(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}
