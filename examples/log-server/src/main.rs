#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(baremetal)]
#[macro_use]
mod debug;

mod log_string;

use core::fmt::Write;
use log_string::LogString;

#[cfg(not(target_os = "none"))]
mod implementation {
    use core::fmt::{Error, Write};
    // use pancurses::{endwin, initscr, Window};
    use std::sync::mpsc::{channel, Receiver, Sender};

    enum ControlMessage {
        Text(String),
        Exit,
    }

    pub struct Output {
        // window: Option<Window>,
        tx: Sender<ControlMessage>,
        rx: Receiver<ControlMessage>,
    }

    pub fn init() -> Output {
        let (tx, rx) = channel();
        // let window = initscr();
        // window.nodelay(true);

        Output {
            tx,
            rx,
            // window: Some(window),
        }
    }

    impl Output {
        pub fn run(&mut self) {
            loop {
                match self.rx.recv_timeout(std::time::Duration::from_millis(50)) {
                    Ok(msg) => match msg {
                        ControlMessage::Exit => break,
                        ControlMessage::Text(s) => {
                            print!("{}", s);
                            // self.window.as_ref().unwrap().printw(s);
                            // self.window.as_ref().unwrap().refresh();
                        }
                    },
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Calling `getch` refreshes the screen
                        // self.window.as_ref().unwrap().getch();
                    }
                    Err(e) => panic!("Error: {}", e),
                }
            }
        }

        pub fn get_writer(&self) -> OutputWriter {
            OutputWriter {
                tx: self.tx.clone(),
            }
        }
    }

    impl Drop for Output {
        fn drop(&mut self) {
            self.tx.send(ControlMessage::Exit).unwrap();
            // endwin();
        }
    }

    impl Write for Output {
        fn write_str(&mut self, s: &str) -> Result<(), Error> {
            // It would be nice if this worked with &str
            self.tx.send(ControlMessage::Text(s.to_owned())).unwrap();
            Ok(())
        }
    }

    pub struct OutputWriter {
        tx: Sender<ControlMessage>,
    }

    impl Write for OutputWriter {
        fn write_str(&mut self, s: &str) -> Result<(), Error> {
            // It would be nice if this worked with &str
            self.tx.send(ControlMessage::Text(s.to_owned())).unwrap();
            Ok(())
        }
    }
}

#[cfg(target_os = "none")]
mod implementation {
    use core::fmt::{Error, Write};

    pub struct Output {
        // addr: usize,
    }

    pub fn init() -> Output {
        let uart = xous::syscall::map_memory(
            xous::MemoryAddress::new(0xf000_4000),
            None,
            4096,
            xous::MemoryFlags::R | xous::MemoryFlags::W,
        )
        .expect("couldn't map serial port");
        unsafe { crate::debug::DEFAULT_UART_ADDR = uart.as_mut_ptr() as _ };
        println!("Mapped UART @ {:08x}", uart.addr.get());

        println!("Process: map success!");
        crate::debug::DEFAULT.enable_rx();

        println!("Allocating IRQ...");
        xous::syscall::claim_interrupt(4, handle_irq, core::ptr::null_mut::<usize>())
            .expect("couldn't claim interrupt");
        println!("Claimed IRQ 4");
        Output {
            // addr: uart.as_mut_ptr() as usize,
        }
    }

    impl Output {
        pub fn get_writer(&self) -> OutputWriter {
            OutputWriter {  }
        }

        pub fn run(&mut self) {
            loop {
                xous::wait_event();
                // match self.rx.recv_timeout(std::time::Duration::from_millis(50)) {
                //     Ok(msg) => match msg {
                //         ControlMessage::Exit => break,
                //         ControlMessage::Text(s) => {
                //             self.window.as_ref().unwrap().printw(s);
                //             self.window.as_ref().unwrap().refresh();
                //         }
                //     },
                //     Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                //         // Calling `getch` refreshes the screen
                //         self.window.as_ref().unwrap().getch();
                //     }
                //     Err(e) => panic!("Error: {}", e),
                // }
            }
        }
    }

    // use core::panic::PanicInfo;

    // #[panic_handler]
    // fn handle_panic(arg: &PanicInfo) -> ! {
    //     println!("PANIC!");
    //     println!("Details: {:?}", arg);
    //     xous::syscall::wait_event();
    //     loop {}
    // }

    fn handle_irq(irq_no: usize, arg: *mut usize) {
        print!("Handling IRQ {} (arg: {:08x}): ", irq_no, arg as usize);

        while let Some(c) = crate::debug::DEFAULT.getc() {
            print!("0x{:02x}", c);
        }
        println!();
    }

    pub struct OutputWriter {
    }

    impl OutputWriter {
        pub fn putc(&self, c: u8) {
            unsafe {
                let base = crate::debug::DEFAULT_UART_ADDR;
                // Wait until TXFULL is `0`
                while base.add(1).read_volatile() != 0 {}
                base.add(0).write_volatile(c as usize)
            };
        }
    }

    impl Write for OutputWriter {
        fn write_str(&mut self, s: &str) -> Result<(), Error> {
            for c in s.bytes() {
                self.putc(c);
            }
            Ok(())
        }
    }
}

fn reader_thread(mut output: implementation::OutputWriter) {
    writeln!(output, "LOG: Xous Logging Server starting up...").unwrap();

    writeln!(output, "LOG: Starting log server...").unwrap();
    let server_addr = xous::create_server(b"xous-logs-output").unwrap();
    writeln!(output, "LOG: Server listening on address {:?}", server_addr).unwrap();

    let mut counter: usize = 0;
    loop {
        if counter.trailing_zeros() >= 12 {
            writeln!(output, "LOG: Counter tick: {}", counter).unwrap();
        }
        counter += 1;
        writeln!(output, "LOG: Waiting for an event...").unwrap();
        let mut envelope =
            xous::syscall::receive_message(server_addr).expect("couldn't get address");
        writeln!(output, "LOG: Got message envelope: {:?}", envelope).unwrap();
        match &mut envelope.body {
            xous::Message::Scalar(msg) => {
                writeln!(output, "LOG: Scalar message from {}: {:?}", envelope.sender, msg).unwrap();
            }
            xous::Message::BlockingScalar(msg) => {
                writeln!(output, "LOG: BlockingScalar message from {}: {:?}", envelope.sender, msg).unwrap();
            }
            xous::Message::Move(msg) => {
                let log_entry = LogString::from_message(msg);
                writeln!(
                    output,
                    "LOG: Moved log  message from {}: {}",
                    envelope.sender, log_entry
                )
                .unwrap();
            }
            xous::Message::Borrow(msg) => {
                let log_entry = LogString::from_message(msg);
                writeln!(
                    output,
                    "LOG: Immutably borrowed log message from {}: {}",
                    envelope.sender, log_entry
                )
                .unwrap();
            }
            xous::Message::MutableBorrow(msg) => {
                let mut log_entry = LogString::from_message(msg);
                writeln!(
                    output,
                    "LOG: Immutably borrowed log message from {}: {}",
                    envelope.sender, log_entry
                )
                .unwrap();
                writeln!(log_entry, " << HELLO FROM THE SERVER").unwrap();
            }
        }
    }
}

#[xous::xous_main]
fn some_main() -> ! {
    let mut output = implementation::init();
    let writer = output.get_writer();
    // xous::arch::ensure_connection().unwrap();
    println!("Creating the reader thread");
    xous::create_thread_simple(reader_thread, writer).unwrap();
    println!("Running the output");
    output.run();
    panic!("exited");
}
