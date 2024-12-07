async fn serve(req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {


}

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([0, 0, 0, 0], 3003));
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(serve))
            .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
