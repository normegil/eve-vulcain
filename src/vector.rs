use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UnicityError {
    #[error("Too much elements in vec: {msg}")]
    TooMuchElements { msg: String },
    #[error("Not enough elements in vec: {msg}")]
    NotEnoughElements { msg: String },
}

pub trait UniqueElement<T> {
    fn unique(&mut self, msg: &str) -> Result<T, UnicityError>;
    fn unique_ref(&self, msg: &str) -> Result<&T, UnicityError>;
}

impl<T> UniqueElement<T> for Vec<T> {
    fn unique(&mut self, msg: &str) -> Result<T, UnicityError> {
        match self.len() {
            0 => Err(UnicityError::NotEnoughElements {
                msg: msg.to_string(),
            }),
            x if x > 1 => Err(UnicityError::TooMuchElements {
                msg: msg.to_string(),
            }),
            _ => Ok(self.remove(0)),
        }
    }

    fn unique_ref(&self, msg: &str) -> Result<&T, UnicityError> {
        match self.len() {
            0 => Err(UnicityError::NotEnoughElements {
                msg: msg.to_string(),
            }),
            x if x > 1 => Err(UnicityError::TooMuchElements {
                msg: msg.to_string(),
            }),
            _ => Ok(&self[0]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unique_single_element() {
        let mut vec = vec![42];
        let result = vec.unique("Single element");

        assert_eq!(result, Ok(42));
        assert!(vec.is_empty());
    }

    #[test]
    fn test_unique_multiple_elements_error() {
        let mut vec = vec![1, 2, 3];
        let result = vec.unique("Multiple elements");

        assert_eq!(
            result,
            Err(UnicityError::TooMuchElements {
                msg: "Multiple elements".to_string()
            })
        );
        assert_eq!(vec, [1, 2, 3]);
    }

    #[test]
    fn test_unique_no_element_error() {
        let mut vec: Vec<i32> = vec![];
        let result = vec.unique("No element");

        assert_eq!(
            result,
            Err(UnicityError::NotEnoughElements {
                msg: "No element".to_string()
            })
        );
        assert!(vec.is_empty());
    }

    #[test]
    fn test_unique_ref_single_element() {
        let vec = vec![42];
        let result = vec.unique_ref("Single element");

        assert_eq!(result, Ok(&42));
    }

    #[test]
    fn test_unique_ref_multiple_elements_error() {
        let vec = vec![1, 2, 3];
        let result = vec.unique_ref("Multiple elements");

        assert_eq!(
            result,
            Err(UnicityError::TooMuchElements {
                msg: "Multiple elements".to_string()
            })
        );
    }

    #[test]
    fn test_unique_ref_no_element_error() {
        let vec: Vec<i32> = vec![];
        let result = vec.unique_ref("No element");

        assert_eq!(
            result,
            Err(UnicityError::NotEnoughElements {
                msg: "No element".to_string()
            })
        );
    }
}
