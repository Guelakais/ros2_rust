use std::time::{Duration, SystemTime};

use crate::rcl_bindings::*;

/// An identifier for a publisher in the local context.
///
/// To quote the `rmw` documentation:
///
/// > The identifier uniquely identifies the publisher for the local context, but
/// > it will not necessarily be the same identifier given in other contexts or processes
/// > for the same publisher.
/// > Therefore the identifier will uniquely identify the publisher within your application
/// > but may disagree about the identifier for that publisher when compared to another
/// > application.
/// > Even with this limitation, when combined with the publisher sequence number it can
/// > uniquely identify a message within your local context.
/// > Publisher GIDs generated by the RMW implementation could collide at some point, in which
/// > case it is not possible to distinguish which publisher sent the message.
/// > The details of how GIDs are generated are RMW implementation dependent.
///
/// > It is possible the the RMW implementation needs to reuse a publisher GID,
/// > due to running out of unique identifiers or some other constraint, in which case
/// > the RMW implementation may document what happens in that case, but that
/// > behavior is not defined here.
/// > However, this should be avoided, if at all possible, by the RMW implementation,
/// > and should be unlikely to happen in practice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublisherGid {
    /// Bytes identifying a publisher in the RMW implementation.
    pub data: [u8; RMW_GID_STORAGE_SIZE],
    /// A string containing the RMW implementation's name.
    ///
    /// The `data` member only uniquely identifies the publisher within
    /// this RMW implementation.
    ///
    /// It is not converted to a [`CString`][1], since most people who request a `MessageInfo`
    /// do not need it.
    ///
    /// [1]: std::ffi::CString
    pub implementation_identifier: *const std::os::raw::c_char,
}

// SAFETY: The implementation identifier doesn't care about which thread it's read from.
unsafe impl Send for PublisherGid {}
// SAFETY: A char does not have interior mutability.
unsafe impl Sync for PublisherGid {}

/// Additional information about a received message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageInfo {
    /// Time when the message was published by the publisher.
    ///
    /// The `rmw` layer does not specify the exact point at which the RMW implementation
    /// must take the timestamp, but it should be taken consistently at the same point in the
    /// process of publishing a message.
    pub source_timestamp: Option<SystemTime>,
    /// Time when the message was received by the subscription.
    ///
    /// The `rmw` layer does not specify the exact point at which the RMW implementation
    /// must take the timestamp, but it should be taken consistently at the same point in the
    /// process of receiving a message.
    pub received_timestamp: Option<SystemTime>,
    /// Sequence number of the received message set by the publisher.
    ///
    /// This sequence number is set by the publisher and therefore uniquely identifies
    /// a message when combined with the publisher GID.
    /// For long running applications, the sequence number might wrap around at some point.
    ///
    /// If the RMW implementation doesn't support sequence numbers, its value will be
    /// [`u64::MAX`].
    ///
    /// Requirements:
    ///
    /// If `psn1` and `psn2` are the publication sequence numbers received together with two messages,
    /// where `psn1` was obtained before `psn2` and both
    /// sequence numbers are from the same publisher (i.e. also same publisher gid), then:
    ///
    /// - `psn2 > psn1` (except in the case of a wrap around)
    /// - `psn2 - psn1 - 1` is the number of messages the publisher sent in the middle of both
    ///   received messages.
    ///   Those might have already been taken by other messages that were received in between or lost.
    ///   `psn2 - psn1 - 1 = 0` if and only if the messages were sent by the publisher consecutively.
    pub publication_sequence_number: u64,
    /// Sequence number of the received message set by the subscription.
    ///
    /// This sequence number is set by the subscription regardless of which
    /// publisher sent the message.
    /// For long running applications, the sequence number might wrap around at some point.
    ///
    /// If the RMW implementation doesn't support sequence numbers, its value will be
    /// [`u64::MAX`].
    ///
    /// Requirements:
    ///
    /// If `rsn1` and `rsn2` are the reception sequence numbers received together with two messages,
    /// where `rsn1` was obtained before `rsn2`, then:
    ///
    /// - `rsn2 > rsn1` (except in the case of a wrap around)
    /// - `rsn2 = rsn1 + 1` if and only if both messages were received consecutively.
    pub reception_sequence_number: u64,
    /// An identifier for the publisher that sent the message.
    pub publisher_gid: PublisherGid,
}

impl MessageInfo {
    pub(crate) fn from_rmw_message_info(rmw_message_info: &rmw_message_info_t) -> Self {
        let source_timestamp = match rmw_message_info.source_timestamp {
            0 => None,
            ts if ts < 0 => Some(SystemTime::UNIX_EPOCH - Duration::from_nanos(ts.unsigned_abs())),
            ts => Some(SystemTime::UNIX_EPOCH + Duration::from_nanos(ts.unsigned_abs())),
        };
        let received_timestamp = match rmw_message_info.received_timestamp {
            0 => None,
            ts if ts < 0 => Some(SystemTime::UNIX_EPOCH - Duration::from_nanos(ts.unsigned_abs())),
            ts => Some(SystemTime::UNIX_EPOCH + Duration::from_nanos(ts.unsigned_abs())),
        };
        let publisher_gid = PublisherGid {
            data: rmw_message_info.publisher_gid.data,
            implementation_identifier: rmw_message_info.publisher_gid.implementation_identifier,
        };
        Self {
            source_timestamp,
            received_timestamp,
            publication_sequence_number: rmw_message_info.publication_sequence_number,
            reception_sequence_number: rmw_message_info.reception_sequence_number,
            publisher_gid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_durations() {
        let rmw_message_info = rmw_message_info_t {
            source_timestamp: -1_000_000_000,
            received_timestamp: 1_000_000_000,
            publication_sequence_number: 0,
            reception_sequence_number: 0,
            publisher_gid: rmw_gid_t {
                data: [0; RMW_GID_STORAGE_SIZE],
                implementation_identifier: std::ptr::null(),
            },
            from_intra_process: false,
        };
        let message_info = MessageInfo::from_rmw_message_info(&rmw_message_info);
        assert_eq!(
            message_info.source_timestamp.unwrap() + Duration::from_nanos(2_000_000_000),
            message_info.received_timestamp.unwrap()
        );
    }

    #[test]
    fn traits() {
        use crate::test_helpers::*;

        assert_send::<MessageInfo>();
        assert_sync::<MessageInfo>();
    }
}
