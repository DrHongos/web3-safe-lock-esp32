Web3 lock on a ESP32

Open a lock* IF YOU ARE OWNER OF A [GNOSIS SAFE](https://safe.global/) 

* here represented by a led on GPIO19 :D (but could be any electronic lock)

Notes:
- I am using a generic ESP32 dev kit
- This implementation uses std
- All commands are based on an OS: Ubuntu 22 
- Project created using `cargo generate esp-rs/esp-idf-template cargo`
- uses browser **window.ethereum** to connect 

Requirements:
- Get into [esp on rust book](https://esp-rs.github.io/book/installation/index.html)
- Modify config.rs with the data required
- a **metamask** browser wallet in the device used to connect to the esp32

Flash: 
Once your esp32 is connected to USB, you can flash it with `cargo r`

Operate:
Your esp32 will connect to your local wifi network. The serial will return its internal IP (something like 192.168.0.x).
Connect to it from another device (connected in the same network) and it will display a served app.
Connect your wallet and set a name if you want, select SUBMIT and sign the message prompted.
The program will **validate your address with the signed message** and then **check if you are an owner of the safe contract**. 