use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    net::SocketAddr,
    time::Duration,
};

use core::ffi::c_void;

use libcoap_rs::{
    message::{CoapMessageCommon, CoapRequest},
    protocol::{CoapMessageCode, CoapMessageType, CoapRequestCode, CoapResponseCode},
    session::{CoapClientSession, CoapSessionCommon},
    types::CoapUri,
    CoapContext, OscoreConf,
};

// INFO: EXAMPLE IMPLEMENTATION OF save_seq_num_func
// This example uses std and fs to save the provided seq_num to a file.
// You are advised to provided your own implementation for embedded environments.
// WARNING: Writing the sequence number to flash every time may harm the lifetime of the storage!
extern "C" fn save_seq_num(seq_num: u64, _param: *mut c_void) -> i32 {
    let mut oscore_seq_safe_file = match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("oscore.seq")
    {
        Ok(file) => file,
        Err(_) => return 0,
    };

    // TODO: refactor this
    if let Err(_) = writeln!(oscore_seq_safe_file, "{}\n", seq_num) {
        return 0;
    }
    if let Err(_) = oscore_seq_safe_file.flush() {
        return 0;
    }

    #[cfg(debug_assertions)]
    println!("DEBUG: Saving sequence number: {}", seq_num);

    1
}

// INFO: EXAMPLE IMPLEMENTATION TO READ LAST SEQUENCE NUMBER
// This example used std and fs to retrieve the last known used sequence number from a file.
// You are advised to provide your own implementation for embedded environments.
fn read_initial_seq_num() -> Option<u64> {
    let file = match File::open("oscore.seq") {
        Ok(f) => f,
        Err(_) => return None,
    };

    let mut reader = BufReader::new(file);

    let mut line = String::new();
    if reader.read_line(&mut line).is_ok() {
        return match line.trim().parse() {
            Ok(num) => Some(num),
            Err(_) => None,
        };
    }
    None
}

// INFO: EXAMPLE TO MANIPULATE CONFIG BYTES WITH NEGOTIATED EDHOC CREDENTIALS
// This example illustrates a way to manipulate existing oscore config bytes with the updated
// credentials (secret, salt, recipient_id) using the edhoc key exchange.
// TODO: We may provide a function function to directly create oscore config bytes from this
// parameters but due to the flexibility and optinality of some keywords we have currently decided
// against it: https://libcoap.net/doc/reference/4.3.5/man_coap-oscore-conf.html
fn edhoc(bytes: Vec<u8>, secret: &str, salt: &str, recipient_id: &str) -> Vec<u8> {
    let mut lines: Vec<String> = core::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|line| line.to_string())
        .collect();
    let mut recipient_found = false;
    for line in lines.iter_mut() {
        if line.starts_with("master_secret") {
            *line = format!("master_secret,hex,\"{}\"", secret);
        } else if line.starts_with("master_salt") {
            *line = format!("master_salt,hex,\"{}\"", salt);
        } else if line.starts_with("recipient_id") {
            *line = format!("recipient_id,ascii,\"{}\"", recipient_id);
            recipient_found = true;
        }
    }
    if !recipient_found {
        for i in 0..lines.len() {
            if lines[i].starts_with("sender_id") {
                lines.insert(i + 1, format!("recipient_id,ascii,\"{}\"", recipient_id));
            }
        }
    }
    let lines = lines.join("\n");

    #[cfg(debug_assertions)]
    println!("{}", lines);
    lines.into_bytes()
}

fn main() {
    let server_address: SocketAddr = "[::1]:5683".parse().unwrap();

    // Create a new context.
    let mut context = CoapContext::new().expect("Failed to create CoAP context");

    // INFO: READ OSCORE CONFIG
    // By default we recommend reading the oscore_conf as bytes from a file using fs.
    // For embedded environments you're advised to provide your own implementation for
    // creating the oscore config bytes as std or fs may not be available.
    let bytes = fs::read("oscore_conf").expect("Could not read oscore_conf file");

    // INFO: EDHOC EXAMPLE
    // The edhoc-function currently offers an easy way to update the secret, salt and recipient_id
    // of a given config file to provide own values negotiated by EDHOC.
    let bytes = edhoc(bytes, "1234", "4321", "device");

    // INFO: CHOOSE AN INITIAL SEQUENCE NUMBER
    // The read_initial_seq_num-function is used to try to read the last saved sequence number from
    // a file using std. It is advised to implement your own logic for retrieving this number,
    // especially for embedded environments as std or fs may not be available.
    let seq_initial = read_initial_seq_num().unwrap_or(1);

    // INFO: CREATE OSCORE CONFIG
    // Now you can use the oscore config bytes generated and the initial sequence number to create
    // an OscoreConf to use with libcoap-rs. You also have to provide a save_seq_num-function which
    // provides logic to save the current sequence number somewhere. We currently provide an
    // example implementation which saves the last sequence number to a file using fs.
    // WARNING: You are advised to provide your own implement for the save_seq_num_func as
    // especially for embedded environments std and fs may not be available. You must also consider
    // writing the sequence number to flash on every time may harm the lifetime of this storage!
    let oscore_conf =
        OscoreConf::new(seq_initial, &bytes, save_seq_num).expect("Could not create oscore_conf");

    // Connect to the server at the specified address over UDP+OSCORE!
    let session = CoapClientSession::connect_oscore(&mut context, server_address, oscore_conf)
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

    #[cfg(debug_assertions)]
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
            #[cfg(debug_assertions)]
            println!("DEBUG: Received valid response");
            return;
        }
    }
}
