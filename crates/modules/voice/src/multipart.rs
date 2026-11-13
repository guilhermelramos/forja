//! Corpo `multipart/form-data` para o envio do áudio. Campos de texto e um arquivo. Feito à mão
//! porque são poucos campos e o formato é simples; o teste compara com um arquivo de fixture.

/// Montador de um corpo multipart com um limite (`boundary`) fixo.
pub struct Multipart {
    boundary: String,
    body: Vec<u8>,
}

impl Multipart {
    pub fn new(boundary: &str) -> Self {
        Self {
            boundary: boundary.to_owned(),
            body: Vec::new(),
        }
    }

    /// Um campo de texto.
    pub fn text(mut self, name: &str, value: &str) -> Self {
        self.open();
        self.body
            .extend_from_slice(format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes());
        self.body.extend_from_slice(value.as_bytes());
        self.body.extend_from_slice(b"\r\n");
        self
    }

    /// Um arquivo, com o nome e o tipo informados.
    pub fn file(mut self, name: &str, filename: &str, content_type: &str, bytes: &[u8]) -> Self {
        self.open();
        self.body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n"
            )
            .as_bytes(),
        );
        self.body.extend_from_slice(bytes);
        self.body.extend_from_slice(b"\r\n");
        self
    }

    /// Fecha o corpo e devolve o `Content-Type` com o limite e os bytes.
    pub fn finish(mut self) -> (String, Vec<u8>) {
        self.body
            .extend_from_slice(format!("--{}--\r\n", self.boundary).as_bytes());
        let content_type = format!("multipart/form-data; boundary={}", self.boundary);
        (content_type, self.body)
    }

    fn open(&mut self) {
        self.body
            .extend_from_slice(format!("--{}\r\n", self.boundary).as_bytes());
    }
}

#[cfg(test)]
mod tests;
