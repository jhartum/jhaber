use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

type ClientMap = Arc<Mutex<HashMap<String, TcpStream>>>;

fn register_client(stream: &TcpStream, clients: &ClientMap) -> Option<String> {
    let mut client_id = String::new();
    let mut buf_reader = BufReader::new(stream.try_clone().unwrap());
    buf_reader.read_line(&mut client_id).ok()?;
    let client_id = client_id.trim().to_string();

    let mut clients = clients.lock().unwrap();
    clients.insert(client_id.clone(), stream.try_clone().unwrap());
    println!("Клиенты зарегистрированы: {:?}", clients.keys());

    Some(client_id)
}

fn process_message(
    reader: &mut Reader<BufReader<TcpStream>>,
    buffer: &mut Vec<u8>,
    clients: &ClientMap,
    to: &str,
) {
    let mut body_content = String::new();
    loop {
        match reader.read_event(buffer) {
            Ok(Event::Start(ref e)) if e.name() == b"body" => {}
            Ok(Event::Text(e)) => {
                body_content = String::from_utf8(e.unescaped().unwrap().to_vec()).unwrap();
            }
            Ok(Event::End(ref e)) if e.name() == b"body" => {
                break;
            }
            Ok(Event::Eof) => {
                println!("Конец потока");
                break;
            }
            Err(e) => {
                eprintln!("Ошибка чтения XML: {:?}", e);
                break;
            }
            _ => (),
        }
    }

    println!("Текст сообщения: {}", body_content);
    // Отправка текста сообщения получателю
    let clients = clients.lock().unwrap();
    if let Some(mut client_stream) = clients.get(to) {
        println!("Отправка сообщения клиенту {}", to);
        client_stream.write_all(body_content.as_bytes()).unwrap();
        println!("Сообщение отправлено клиенту {}", to);
    } else {
        println!("Клиент {} не найден", to);
    }
}

fn handle_client(mut stream: TcpStream, clients: ClientMap) {
    let mut buffer = Vec::new();
    let mut reader = Reader::from_reader(BufReader::new(stream.try_clone().unwrap()));
    reader.trim_text(true);

    println!("Клиент подключен");

    if let Some(client_id) = register_client(&stream, &clients) {
        loop {
            buffer.clear();
            match reader.read_event(&mut buffer) {
                Ok(Event::Start(ref e)) if e.name() == b"stream:stream" => {
                    println!("Получен начальный поток");
                    let response = r#"<?xml version='1.0'?><stream:stream from='localhost' xmlns='jabber:client' xmlns:stream='http://etherx.jabber.org/streams' version='1.0'>"#;
                    stream.write_all(response.as_bytes()).unwrap();
                    println!("Ответ отправлен");
                }
                Ok(Event::Start(ref e)) if e.name() == b"message" => {
                    if let Some(to_attr) = e
                        .attributes()
                        .with_checks(false)
                        .find(|attr| attr.as_ref().unwrap().key == b"to")
                    {
                        let to_value = to_attr
                            .unwrap()
                            .unescaped_value()
                            .unwrap()
                            .to_owned()
                            .into_owned();
                        let to = String::from_utf8(to_value).unwrap();
                        process_message(&mut reader, &mut buffer, &clients, &to);
                    }
                }
                Ok(Event::End(ref e)) => {
                    println!("Закрывающий тег: {:?}", e.name());
                }
                Ok(Event::Eof) => {
                    println!("Конец потока");
                    break;
                }
                Err(e) => {
                    eprintln!("Ошибка чтения XML: {:?}", e);
                    break;
                }
                _ => (),
            }
        }

        let mut clients = clients.lock().unwrap();
        clients.remove(&client_id);
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:5222").unwrap();
    println!("Сервер запущен на 127.0.0.1:5222");

    let clients: ClientMap = Arc::new(Mutex::new(HashMap::new()));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let clients = Arc::clone(&clients);
                thread::spawn(move || {
                    handle_client(stream, clients);
                });
            }
            Err(e) => {
                eprintln!("Ошибка соединения: {}", e);
            }
        }
    }
}
