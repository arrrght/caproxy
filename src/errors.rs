pub enum CheckersErr {
    Reqwest(reqwest::Error),
    Other(String),
    Io(std::io::Error),
    Num(std::num::ParseIntError),
}

impl From<std::num::ParseIntError> for CheckersErr {
    fn from(e: std::num::ParseIntError) -> CheckersErr {
        CheckersErr::Num(e)
    }
}

impl From<reqwest::Error> for CheckersErr {
    fn from(e: reqwest::Error) -> CheckersErr {
        CheckersErr::Reqwest(e)
    }
}

impl From<std::io::Error> for CheckersErr {
    fn from(e: std::io::Error) -> CheckersErr {
        CheckersErr::Io(e)
    }
}

impl std::fmt::Debug for CheckersErr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            CheckersErr::Other(ref err) => write!(f, "Other: {}", err),
            CheckersErr::Reqwest(ref err) => write!(f, "Reqwest: {:?}", err),
            CheckersErr::Io(ref err) => write!(f, "Io: {:?}", err),
            CheckersErr::Num(ref err) => write!(f, "Num: {:?}", err),
        }
    }
}
