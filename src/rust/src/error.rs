// This file is dual licensed under the terms of the Apache License, Version
// 2.0, and the BSD License. See the LICENSE file in the root of this repository
// for complete details.

pub enum CryptographyError {
    Asn1Parse(asn1::ParseError),
    Asn1Write(asn1::WriteError),
    Py(pyo3::PyErr),
    OpenSSL(openssl::error::ErrorStack),
}

impl From<asn1::ParseError> for CryptographyError {
    fn from(e: asn1::ParseError) -> CryptographyError {
        CryptographyError::Asn1Parse(e)
    }
}

impl From<asn1::WriteError> for CryptographyError {
    fn from(e: asn1::WriteError) -> CryptographyError {
        CryptographyError::Asn1Write(e)
    }
}

impl From<pyo3::PyErr> for CryptographyError {
    fn from(e: pyo3::PyErr) -> CryptographyError {
        CryptographyError::Py(e)
    }
}

impl From<pyo3::PyDowncastError<'_>> for CryptographyError {
    fn from(e: pyo3::PyDowncastError<'_>) -> CryptographyError {
        CryptographyError::Py(e.into())
    }
}

impl From<openssl::error::ErrorStack> for CryptographyError {
    fn from(e: openssl::error::ErrorStack) -> CryptographyError {
        CryptographyError::OpenSSL(e)
    }
}

impl From<pem::PemError> for CryptographyError {
    fn from(e: pem::PemError) -> CryptographyError {
        CryptographyError::Py(pyo3::exceptions::PyValueError::new_err(format!(
            "Unable to load PEM file. See https://cryptography.io/en/latest/faq/#why-can-t-i-import-my-pem-file for more details. {:?}",
            e
        )))
    }
}

impl From<CryptographyError> for pyo3::PyErr {
    fn from(e: CryptographyError) -> pyo3::PyErr {
        match e {
            CryptographyError::Asn1Parse(asn1_error) => pyo3::exceptions::PyValueError::new_err(
                format!("error parsing asn1 value: {:?}", asn1_error),
            ),
            CryptographyError::Asn1Write(asn1::WriteError::AllocationError) => {
                pyo3::exceptions::PyMemoryError::new_err(
                    "failed to allocate memory while performing ASN.1 serialization",
                )
            }
            CryptographyError::Py(py_error) => py_error,
            CryptographyError::OpenSSL(error_stack) => {
                let gil = pyo3::Python::acquire_gil();
                let py = gil.python();

                let internal_error = py
                    .import("cryptography.exceptions")
                    .expect("Failed to import cryptography module")
                    .getattr(crate::intern!(py, "InternalError"))
                    .expect("Failed to get InternalError attribute");

                let binding_mod = py
                    .import("cryptography.hazmat.bindings.openssl.binding")
                    .expect("Failed to import cryptography module");

                let openssl_error = binding_mod
                    .getattr(crate::intern!(py, "_OpenSSLError"))
                    .expect("Failed to get _OpenSSL attribute");
                let openssl_error_with_text = binding_mod
                    .getattr(crate::intern!(py, "_OpenSSLErrorWithText"))
                    .expect("Failed to get _OpenSSLErrorWithText attribute");

                let errors = pyo3::types::PyList::empty(py);
                for e in error_stack.errors() {
                    let err = openssl_error
                        .call_method1("from_code", (e.code(),))
                        .expect("Failed to call from_code");

                    errors
                        .append(
                            openssl_error_with_text
                                .call_method1("from_err", (err,))
                                .expect("Failed to call from_err"),
                        )
                        .expect("List append failed");
                }
                pyo3::PyErr::from_instance(
                    internal_error
                        .call1((
                            "Unknown OpenSSL error. This error is commonly encountered
                    when another library is not cleaning up the OpenSSL error
                    stack. If you are using cryptography with another library
                    that uses OpenSSL try disabling it before reporting a bug.
                    Otherwise please file an issue at
                    https://github.com/pyca/cryptography/issues with
                    information on how to reproduce this.",
                            errors,
                        ))
                        .expect("Failed to create InternalError"),
                )
            }
        }
    }
}

impl CryptographyError {
    pub(crate) fn add_location(self, loc: asn1::ParseLocation) -> Self {
        match self {
            CryptographyError::Py(e) => CryptographyError::Py(e),
            CryptographyError::Asn1Parse(e) => CryptographyError::Asn1Parse(e.add_location(loc)),
            CryptographyError::Asn1Write(e) => CryptographyError::Asn1Write(e),
            CryptographyError::OpenSSL(e) => CryptographyError::OpenSSL(e),
        }
    }
}

// The primary purpose of this alias is for brevity to keep function signatures
// to a single-line as a work around for coverage issues. See
// https://github.com/pyca/cryptography/pull/6173
pub(crate) type CryptographyResult<T = pyo3::PyObject> = Result<T, CryptographyError>;

#[cfg(test)]
mod tests {
    use super::CryptographyError;

    #[test]
    fn test_cryptographyerror_from() {
        pyo3::prepare_freethreaded_python();
        pyo3::Python::with_gil(|py| {
            let e: CryptographyError = asn1::WriteError::AllocationError.into();
            assert!(matches!(
                e,
                CryptographyError::Asn1Write(asn1::WriteError::AllocationError)
            ));
            let py_e: pyo3::PyErr = e.into();
            assert!(py_e.is_instance::<pyo3::exceptions::PyMemoryError>(py));

            let e: CryptographyError =
                pyo3::PyDowncastError::new(py.None().as_ref(py), "abc").into();
            assert!(matches!(e, CryptographyError::Py(_)));
        })
    }

    #[test]
    fn test_cryptographyerror_add_location() {
        let py_err = pyo3::PyErr::new::<pyo3::exceptions::PyValueError, _>("Error!");
        CryptographyError::Py(py_err).add_location(asn1::ParseLocation::Field("meh"));

        let asn1_write_err = asn1::WriteError::AllocationError;
        CryptographyError::Asn1Write(asn1_write_err)
            .add_location(asn1::ParseLocation::Field("meh"));

        let openssl_error = openssl::error::ErrorStack::get();
        CryptographyError::from(openssl_error).add_location(asn1::ParseLocation::Field("meh"));
    }
}
