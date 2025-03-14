use std::io::Read;

pub fn send_request(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut response = reqwest::blocking::get(url)?;
    let mut body = String::new();
    tracing::debug!("Requesting url {} ...", url);
    response.read_to_string(&mut body)?;
    tracing::debug!("Requesting url {} done", url);

    // Uncomment these lines if you need to debug the response
    // println!("Status: {}", response.status());
    // println!("Headers:\n{:#?}", response.headers());
    // println!("Body:\n{}", body);

    Ok(body)
}