use std::cmp::max;
use std::convert::Infallible;
use std::io::Cursor;
use std::net::SocketAddr;
use std::time::Duration;
use regex::Regex;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::CONTENT_TYPE;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, Uri};
use hyper_util::rt::TokioIo;
use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
use image::{Delay, DynamicImage, Frame, ImageReader, RgbaImage};
use image::AnimationDecoder;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use resvg::tiny_skia::{Color, Paint, Pixmap, PixmapMut};
use resvg::usvg::{Options, Rect, Transform, Tree};
use tokio::net::TcpListener;
use tokio::time;

struct ImageWithDelay {
    image: DynamicImage,
    delay: Delay
}

fn overlay(mut image: DynamicImage, svg: &Tree) -> DynamicImage {
    let size = max(image.width(), image.height());
    if size > 500 {
        image = image.resize(500, 500, image::imageops::FilterType::Nearest);
    }

    let mut pixels = Pixmap::new(image.width(), image.height()).unwrap();

    let scale = image.width() as f32 / 261f32;
    let ratio = image.width() as f32 / image.height() as f32;
    let mut vscale = scale * 0.7;
    let mut offset_y = 0i64;

    if ratio > 1.0 {
        vscale *= 0.8;
        vscale /= ratio;
    }
    else
    {
        offset_y = (scale * 20.0) as i64;

        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(53, 57, 63, 255));
        pixels.fill_rect(Rect::from_xywh(0.0, 0.0, pixels.width() as f32, offset_y as f32).unwrap(), &paint, Transform::from_scale(1.0, 1.0), None);
    }

    let mut pixels = PixmapMut::from_bytes(pixels.data_mut(), image.width(), image.height()).unwrap();

    let render_transform = Transform::from_scale(scale, scale.min(vscale)).post_translate(0.0, offset_y as f32);
    resvg::render(svg, render_transform, &mut pixels);

    let svg_image = DynamicImage::ImageRgba8(RgbaImage::from_raw(pixels.width(), pixels.height(), pixels.data_mut().to_vec()).unwrap());

    image::imageops::overlay(&mut image, &svg_image, 0, 0);

    image
}

async fn handle_tenor(response: reqwest::Response) -> reqwest::Response {
    let html = response.text().await.unwrap();
    
    let pattern = Regex::new(r#".*?<img src=\x22(https://media.*?)\x22.*"#).unwrap();
    let url = pattern.captures(&html).unwrap().get(1).unwrap().as_str();

    reqwest::get(url).await.unwrap()
}

async fn serve(req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let allowed: Vec<String> = [
        "cdn.discordapp.com",
        "tenor.com",
        "media1.tenor.com"
    ].map(|s| s.to_string()).to_vec();

    let uri = req.uri().to_string();
    let uri = uri.strip_prefix("/").unwrap();
    let image_uri: Uri = uri.parse().unwrap();

    if allowed.contains(&image_uri.host().unwrap().to_string()) {
        let mut response = reqwest::get(image_uri.to_string()).await.unwrap();

        if image_uri.host().unwrap() == "tenor.com" {
            response = handle_tenor(response).await;
        }

        let content_type = response.headers().get(CONTENT_TYPE);

        if let Some(content_type) = content_type {
            let content_type = content_type.clone();

            if content_type.to_str().unwrap().contains("image") {
                let img_bytes = response.bytes().await.unwrap();

                let svg_data = std::fs::read("./bubble.svg").unwrap();
                let rtree = Tree::from_data(&svg_data, &Options::default()).unwrap();
                let mut images: Vec<ImageWithDelay>;

                if content_type == "image/gif" {
                    let gif = GifDecoder::new(Cursor::new(img_bytes)).unwrap();
                    let frames: Vec<Result<Frame, image::ImageError>> = gif.into_frames().collect();

                    images = frames.par_iter().filter_map(|frame| {
                        if let Ok(frame) = frame {
                            let delay = frame.delay();

                            let image = DynamicImage::ImageRgba8(frame.buffer().clone());

                            Some(ImageWithDelay { image: overlay(image, &rtree), delay })
                        }
                        else {
                            None
                        }

                    }).collect();
                } else {
                    images = Vec::new();

                    let image = ImageReader::new(Cursor::new(img_bytes))
                        .with_guessed_format().unwrap().decode().unwrap();

                    let delay = Delay::from_saturating_duration(Duration::from_millis(1));
                    images.push(ImageWithDelay { image: overlay(image, &rtree), delay });
                }

                let mut img_out_bytes: Vec<u8> = Vec::new();
                {
                    let mut cursor = Cursor::new(&mut img_out_bytes);
                    let mut encoder = GifEncoder::new_with_speed(&mut cursor, 30);
                    encoder.set_repeat(Repeat::Infinite).unwrap();

                    for img in images.into_iter() {
                        encoder.encode_frame(Frame::from_parts(img.image.to_rgba8(), 0, 0, img.delay)).unwrap();
                    }
                }
                return Ok(Response::new(Full::from(Bytes::from(img_out_bytes))));

            }
        }
    }

    Ok(Response::new(Full::new(Bytes::from("nie lubie cie"))))
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 3003));
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            let result = time::timeout(Duration::new(15, 0), 
                http1::Builder::new().serve_connection(io, service_fn(serve)
            )).await;
            if let Err(err) = result {
                println!("Error serving connection (timeout?): {:?}", err);
            }
        });
    }
}
