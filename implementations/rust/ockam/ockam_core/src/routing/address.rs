use crate::access_control::IncomingAccessControl;
use crate::compat::rand::{distributions::Standard, prelude::Distribution, random, Rng};
use crate::compat::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use crate::{debugger, DenyAll, OutgoingAccessControl, RelayMessage, Result};
use core::cmp::Ordering;
use core::fmt::{self, Debug, Display};
use core::ops::Deref;
use core::str::from_utf8;
use serde::{Deserialize, Serialize};

/// A `Mailbox` controls the dispatch of incoming messages for a
/// particular [`Address`]
#[derive(Clone)]
pub struct Mailbox {
    address: Address,
    incoming: Arc<dyn IncomingAccessControl>,
    outgoing: Arc<dyn OutgoingAccessControl>,
}

impl Debug for Mailbox {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {{in:{:?} out:{:?}}}",
            self.address, self.incoming, self.outgoing
        )
    }
}

impl Ord for Mailbox {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.address.cmp(&rhs.address)
    }
}
impl PartialOrd for Mailbox {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}
impl PartialEq for Mailbox {
    fn eq(&self, rhs: &Self) -> bool {
        self.address == rhs.address
    }
}
impl Eq for Mailbox {}

impl Mailbox {
    /// Create a new `Mailbox` with the given [`Address`], [`IncomingAccessControl`] and [`OutgoingAccessControl`]
    pub fn new(
        address: impl Into<Address>,
        incoming: Arc<dyn IncomingAccessControl>,
        outgoing: Arc<dyn OutgoingAccessControl>,
    ) -> Self {
        Self {
            address: address.into(),
            incoming,
            outgoing,
        }
    }
    /// Create a new `Mailbox` allowed to send and receive all messages
    pub fn deny_all(address: impl Into<Address>) -> Self {
        Self {
            address: address.into(),
            incoming: Arc::new(DenyAll),
            outgoing: Arc::new(DenyAll),
        }
    }
    /// Return a reference to the [`Address`] of this mailbox
    pub fn address(&self) -> &Address {
        &self.address
    }
    /// Return a reference to the [`IncomingAccessControl`] for this mailbox
    pub fn incoming_access_control(&self) -> &Arc<dyn IncomingAccessControl> {
        &self.incoming
    }
    /// Return a reference to the [`OutgoingAccessControl`] for this mailbox
    pub fn outgoing_access_control(&self) -> &Arc<dyn OutgoingAccessControl> {
        &self.outgoing
    }
}

/// A collection of [`Mailbox`]es for a [`Context`]
#[derive(Clone)]
pub struct Mailboxes {
    main_mailbox: Mailbox,
    additional_mailboxes: Vec<Mailbox>,
}

impl Debug for Mailboxes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:?} + {:?}",
            self.main_mailbox, self.additional_mailboxes
        )
    }
}

impl Mailboxes {
    /// Create a new collection of `Mailboxes` from the given [`Mailbox`]es
    pub fn new(main_mailbox: Mailbox, additional_mailboxes: Vec<Mailbox>) -> Self {
        Self {
            main_mailbox,
            additional_mailboxes,
        }
    }

    /// Create a new collection of `Mailboxes` for the given
    /// [`Address`] with [`IncomingAccessControl`] and [`OutgoingAccessControl`]
    pub fn main(
        address: impl Into<Address>,
        incoming_access_control: Arc<dyn IncomingAccessControl>,
        outgoing_access_control: Arc<dyn OutgoingAccessControl>,
    ) -> Self {
        Self {
            main_mailbox: Mailbox::new(
                address.into(),
                incoming_access_control,
                outgoing_access_control,
            ),
            additional_mailboxes: vec![],
        }
    }

    /// Return an [`AddressSet`] containing all addresses represented by these `Mailboxes`
    pub fn aliases(&self) -> Vec<Address> {
        self.additional_mailboxes
            .iter()
            .map(|x| x.address.clone())
            .collect()
    }

    /// Return the main [`Address`] for these `Mailboxes`
    pub fn main_address(&self) -> Address {
        self.main_mailbox.address.clone()
    }

    /// Return `true` if the given [`Address`] is represented by these `Mailboxes`
    pub fn contains(&self, msg_addr: &Address) -> bool {
        if &self.main_mailbox.address == msg_addr {
            true
        } else {
            self.additional_mailboxes
                .iter()
                .any(|x| &x.address == msg_addr)
        }
    }

    /// Return a reference to the [`Mailbox`] with the given [`Address`]
    pub fn find_mailbox(&self, msg_addr: &Address) -> Option<&Mailbox> {
        if &self.main_mailbox.address == msg_addr {
            Some(&self.main_mailbox)
        } else {
            self.additional_mailboxes
                .iter()
                .find(|x| &x.address == msg_addr)
        }
    }

    /// Return `true` if the given [`Address`] is authorized to post
    /// the given [`RelayMessage`] to these `Mailboxes`
    /// TODO docs are confusing
    pub async fn is_incoming_authorized(&self, relay_msg: &RelayMessage) -> Result<bool> {
        if let Some(mailbox) = self.find_mailbox(relay_msg.destination()) {
            debugger::log_incoming_access_control(mailbox, relay_msg);

            mailbox.incoming.is_authorized(relay_msg).await
        } else {
            warn!(
                "Message from {} for {} does not match any addresses for this destination",
                relay_msg.source(),
                relay_msg.destination()
            );
            crate::deny()
        }
    }

    /// Return `true` if these `Mailboxes` are authorized to post the
    /// given [`RelayMessage`] to the given [`Address`]
    /// TODO docs are confusing
    pub async fn is_outgoing_authorized(&self, relay_msg: &RelayMessage) -> Result<bool> {
        if let Some(mailbox) = self.find_mailbox(relay_msg.source()) {
            debugger::log_outgoing_access_control(mailbox, relay_msg);

            mailbox.outgoing.is_authorized(relay_msg).await
        } else {
            warn!(
                "Message from {} for {} does not match any addresses for this origin",
                relay_msg.source(),
                relay_msg.destination()
            );
            crate::deny()
        }
    }

    /// Return the [`AddressSet`] represented by these `Mailboxes`
    pub fn addresses(&self) -> Vec<Address> {
        let mut addresses = vec![self.main_mailbox.address.clone()];
        addresses.append(&mut self.aliases());
        addresses
    }

    /// Return a reference to the main [`Mailbox`] for this collection
    /// of `Mailboxes`
    pub fn main_mailbox(&self) -> &Mailbox {
        &self.main_mailbox
    }

    /// Return a reference to the additional [`Mailbox`]es for this
    /// collection of `Mailboxes`
    pub fn additional_mailboxes(&self) -> &Vec<Mailbox> {
        &self.additional_mailboxes
    }
}

/// A generic address type.
///
/// The address type is parsed by routers to determine the next local
/// hop in the router chain to resolve a route to a remote connection.
///
/// ## Parsing addresses
///
/// While addresses are concrete types, creating them from strings is
/// possible for ergonomic reasons.
///
/// When parsing an address from a string, the first `#` symbol is
/// used to separate the transport type from the rest of the address.
/// If no `#` symbol is found, the address is assumed to be of `transport =
/// 0`, the Local Worker transport type.
///
/// For example:
/// * `"0#alice"` represents a local worker with the address: `alice`.
/// * `"1#carol"` represents a remote worker with the address `carol`, reachable over TCP transport.
///
#[derive(Serialize, Deserialize, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct Address {
    tt: TransportType,
    inner: Vec<u8>,
}

/// An error which is returned when address parsing from string fails.
#[derive(Debug)]
pub struct AddressParseError {
    kind: AddressParseErrorKind,
}

/// Enum to store the cause of an address parsing failure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AddressParseErrorKind {
    /// Unable to parse address num in the address string.
    InvalidType(core::num::ParseIntError),
    /// Address string has more than one '#' separator.
    MultipleSep,
}

impl AddressParseError {
    /// Create new address parse error instance.
    pub fn new(kind: AddressParseErrorKind) -> Self {
        Self { kind }
    }
    /// Return the cause of the address parsing failure.
    pub fn kind(&self) -> &AddressParseErrorKind {
        &self.kind
    }
}

impl Display for AddressParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            AddressParseErrorKind::InvalidType(e) => {
                write!(f, "Failed to parse address type: '{}'", e)
            }
            AddressParseErrorKind::MultipleSep => {
                write!(
                    f,
                    "Invalid address string: more than one '#' separator found"
                )
            }
        }
    }
}

impl crate::compat::error::Error for AddressParseError {}

impl Address {
    /// Creates a new address from separate transport type and data parts.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ockam_core::{Address, TransportType};
    /// # pub const TCP: TransportType = TransportType::new(1);
    /// // create a new remote worker address from a transport type and data
    /// let tcp_worker: Address = Address::new(TCP, "carol");
    /// ```
    pub fn new<S: Into<String>>(tt: TransportType, data: S) -> Self {
        Self {
            tt,
            inner: data.into().as_bytes().to_vec(),
        }
    }

    /// Parses an address from a string.
    ///
    /// # Panics
    ///
    /// This function will panic if passed an invalid address string.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ockam_core::Address;
    /// // parse a local worker address
    /// let local_worker: Address = Address::from_string("alice");
    ///
    /// // parse a remote worker address reachable over tcp transport
    /// let tcp_worker: Address = Address::from_string("1#carol");
    /// ```
    pub fn from_string<S: Into<String>>(s: S) -> Self {
        match s.into().parse::<Address>() {
            Ok(a) => a,
            Err(e) => {
                panic!("Invalid address string {}", e)
            }
        }
    }

    /// Get the string value of this address without the address type
    #[doc(hidden)]
    pub fn without_type(&self) -> &str {
        from_utf8(&self.inner).unwrap_or("<unprintable>")
    }

    /// Generate a random address with the given transport type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ockam_core::{Address, LOCAL};
    /// // generate a random local address
    /// let local_worker: Address = Address::random(LOCAL);
    /// ```
    pub fn random(tt: TransportType) -> Self {
        Self { tt, ..random() }
    }

    /// Generate a random address with transport type [`LOCAL`].
    pub fn random_local() -> Self {
        Self {
            tt: LOCAL,
            ..random()
        }
    }

    // TODO: Replace with macro to take less space when "debugger" feature is disabled?
    /// Generate a random address with a debug tag and transport type [`LOCAL`].
    pub fn random_tagged(_tag: &str) -> Self {
        #[cfg(feature = "debugger")]
        {
            use core::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let address = format!("{}_{}", _tag, COUNTER.fetch_add(1, Ordering::Relaxed),).into();
            let address = Self {
                tt: LOCAL,
                ..address
            };
            tracing::trace!("random_tagged => {}", address);
            address
        }

        #[cfg(not(feature = "debugger"))]
        Self::random_local()
    }

    /// Get transport type of this address.
    pub fn transport_type(&self) -> TransportType {
        self.tt
    }

    /// Get address portion of this address
    pub fn address(&self) -> &str {
        from_utf8(self.inner.as_slice()).unwrap_or("Invalid UTF-8")
    }

    /// Check if address is local
    pub fn is_local(&self) -> bool {
        self.tt == LOCAL
    }
}

impl core::str::FromStr for Address {
    type Err = AddressParseError;
    /// Parse an address from a string.
    ///
    /// See type documentation for more detail.
    fn from_str(s: &str) -> Result<Address, Self::Err> {
        let buf: String = s.into();
        let mut vec: Vec<_> = buf.split('#').collect();

        // If after the split we only have one element, there was no
        // `#` separator, so the type needs to be implicitly `= 0`
        if vec.len() == 1 {
            Ok(Address {
                tt: LOCAL,
                inner: vec.remove(0).as_bytes().to_vec(),
            })
        }
        // If after the split we have 2 elements, we extract the type
        // value from the string, and use the rest as the address
        else if vec.len() == 2 {
            match str::parse(vec.remove(0)) {
                Ok(tt) => Ok(Address {
                    tt: TransportType::new(tt),
                    inner: vec.remove(0).as_bytes().to_vec(),
                }),
                Err(e) => Err(AddressParseError::new(AddressParseErrorKind::InvalidType(
                    e,
                ))),
            }
        } else {
            Err(AddressParseError::new(AddressParseErrorKind::MultipleSep))
        }
    }
}

impl Display for Address {
    fn fmt<'a>(&'a self, f: &mut fmt::Formatter) -> fmt::Result {
        let inner: &'a str = from_utf8(self.inner.as_slice()).unwrap_or("Invalid UTF-8");
        write!(f, "{}#{}", self.tt, inner)
    }
}

impl Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <Self as Display>::fmt(self, f)
    }
}

impl Deref for Address {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl From<String> for Address {
    fn from(s: String) -> Self {
        Self::from_string(s)
    }
}

impl From<&String> for Address {
    fn from(s: &String) -> Self {
        Self::from_string(s.as_str())
    }
}

impl<'a> From<&'a str> for Address {
    fn from(s: &'a str) -> Self {
        Self::from_string(s)
    }
}

impl From<Vec<u8>> for Address {
    fn from(data: Vec<u8>) -> Self {
        Self {
            tt: LOCAL,
            inner: data,
        }
    }
}

impl From<(TransportType, Vec<u8>)> for Address {
    fn from((tt, data): (TransportType, Vec<u8>)) -> Self {
        Self { tt, inner: data }
    }
}

impl<'a> From<(TransportType, &'a str)> for Address {
    fn from((tt, data): (TransportType, &'a str)) -> Self {
        Self {
            tt,
            inner: data.as_bytes().to_vec(),
        }
    }
}

impl<'a> From<(TransportType, &'a String)> for Address {
    fn from((tt, inner): (TransportType, &'a String)) -> Self {
        Self::from((tt, inner.as_str()))
    }
}

impl From<(TransportType, String)> for Address {
    fn from((transport, data): (TransportType, String)) -> Self {
        Self::from((transport, data.as_str()))
    }
}

impl<'a> From<&'a [u8]> for Address {
    fn from(data: &'a [u8]) -> Self {
        Self {
            tt: LOCAL,
            inner: data.to_vec(),
        }
    }
}

impl<'a> From<&'a [&u8]> for Address {
    fn from(data: &'a [&u8]) -> Self {
        Self {
            tt: LOCAL,
            inner: data.iter().map(|x| **x).collect(),
        }
    }
}

impl From<Address> for String {
    fn from(address: Address) -> Self {
        address.to_string()
    }
}

impl Distribution<Address> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Address {
        let address: [u8; 16] = rng.gen();
        hex::encode(address).as_bytes().into()
    }
}

/// The transport type of an address.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct TransportType(u8);

/// The local transport type.
pub const LOCAL: TransportType = TransportType::new(0);

impl TransportType {
    /// Create a new transport type.
    pub const fn new(n: u8) -> Self {
        TransportType(n)
    }

    /// Is this the local transport type?
    pub fn is_local(self) -> bool {
        self == LOCAL
    }
}

impl Display for TransportType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<TransportType> for u8 {
    fn from(ty: TransportType) -> Self {
        ty.0
    }
}

#[test]
fn parse_addr_simple() {
    let addr = Address::from_string("local_friend");
    assert_eq!(
        addr,
        Address {
            tt: LOCAL,
            inner: "local_friend".as_bytes().to_vec()
        }
    );
}

#[test]
fn parse_addr_with_type() {
    let addr = Address::from_string("1#remote_friend");
    assert_eq!(
        addr,
        Address {
            tt: TransportType::new(1),
            inner: "remote_friend".as_bytes().to_vec()
        }
    );
}

#[test]
#[should_panic(expected = "Failed to parse address type:")]
fn parse_addr_invalid() {
    Address::from_string("#,my_friend");
}

#[test]
#[should_panic(expected = "Invalid address string:")]
fn parse_addr_invalid_multiple_separators() {
    let _ = Address::from_string("1#invalid#");
}
