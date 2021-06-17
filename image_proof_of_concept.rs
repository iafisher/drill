use termimage::ops::{guess_format, load_image, resize_image, write_ansi_truecolor};
use term_size;


let path = (String::new(), PathBuf::from("/home/iafisher/downloads/test.jpg"));
let format = guess_format(&path).unwrap();
let image = load_image(&path, format).unwrap();
let (w, h) = term_size::dimensions().unwrap();
let image_resized = resize_image(&image, (w as u32, h as u32));
write_ansi_truecolor(&mut io::stdout(), &image_resized);
