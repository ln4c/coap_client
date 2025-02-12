use std::{
    fs,
    net::{SocketAddr, UdpSocket},
    time::Duration,
};

use libcoap_rs::{
    error::UriParsingError,
    message::{CoapMessageCommon, CoapRequest, CoapResponse},
    protocol::{CoapMessageCode, CoapMessageType, CoapRequestCode, CoapResponseCode},
    session::{CoapClientSession, CoapSessionCommon},
    types::{CoapUri, CoapUriScheme},
    CoapContext, CoapRequestHandler, CoapResource,
};

fn main() {
    let server_address: SocketAddr = "[::1]:5683".parse().unwrap();

    // Create a new context.
    let mut context = CoapContext::new().expect("Failed to create CoAP context");

    // read bytes from oscore_conf
    let bytes = fs::read("oscore_conf").expect("ERROR: Could not read oscore_conf");

    // Connect to the server at the specified address over UDP+OSCORE!
    let session = CoapClientSession::connect_oscore(&mut context, server_address, 1, &bytes)
        .expect("Failed to create client-side session");

    // Create a new CoAP URI to request from.
    let uri = CoapUri::try_from_str("coap://[::1]:5683/hello_world");

    // Create a new request of type get with the specified URI.
    let mut request =
        CoapRequest::new(CoapMessageType::Con, CoapRequestCode::Get, uri.unwrap()).unwrap();

    // Send the request and wait for a response.
    let req_handle = session
        .send_request(request)
        .expect("Unable to send request");

    println!("DEBUG: Send hello_world request");

    loop {
        context
            .do_io(Some(Duration::from_secs(10)))
            .expect("error during IO");
        // Poll for responses to a request using the request handle.
        for response in session.poll_handle(&req_handle) {
            assert_eq!(
                response.code(),
                CoapMessageCode::Response(CoapResponseCode::Content)
            );
            assert_eq!(response.data().unwrap().as_ref(), "Hello World!".as_bytes());
            println!("DEBUG: Received valid response");
            return;
        }
    }
}
