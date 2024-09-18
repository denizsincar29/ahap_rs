# ahap_rs
A rust crate to create apple haptic feedback patterns.

## Usage
This crate is still made as binary, so you can clone it and run it directly. It will create an .ahap file with a bike engine sound that you can even open via file manager or send it to your friends via messaging apps.
```bash
git clone https://github.com/denizsincar29/ahap_rs
cd ahap_rs
cargo run --release
```

## Technical Details

Ahap is a json based format that is used to create haptic feedback patterns for Apple devices. The .ahap files can be used in various applications to provide tactile feedback to users. They can be used in games, notifications, and other interactive experiences internally inside the app code.
But since iOS 17 you can view .ahap files directly via iOS's quick look feature, which means they will open from the file manager or messaging apps that support it, e.g. Telegram, Whatsapp, etc.

## License
This project is licensed under the MIT License.

## Contributing
Contributions are welcome! Please feel free to submit a pull request or open an issue.