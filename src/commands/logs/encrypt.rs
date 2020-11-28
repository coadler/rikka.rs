use arrayvec::ArrayString;
use byteorder::{ByteOrder, LittleEndian};
use lazy_static::lazy_static;
use std::convert::TryInto;
use std::env;
use twilight_model::id::{AttachmentId, MessageId};

lazy_static! {
    static ref KEY: [u8; 32] =
        hex::decode(env::var("LOG_HASH_KEY").expect("LOG_HASH_KEY not provided"))
            .unwrap()
            .try_into()
            .expect("LOG_HASH_KEY isn't 32 bytes");
}

pub fn img_hash_secret(mid: &MessageId, aid: &AttachmentId) -> ArrayString<[u8; 64]> {
    let mut hash = blake3::Hasher::new_keyed(&KEY);

    let mut mid_raw = [0u8; 8];
    LittleEndian::write_u64(&mut mid_raw, mid.0);
    hash.update(&mid_raw);

    let mut aid_raw = [0u8; 8];
    LittleEndian::write_u64(&mut aid_raw, aid.0);
    hash.update(&aid_raw);

    hash.finalize().to_hex()
}

// #[derive(Serialize, Deserialize)]
// pub struct EncryptedMessage {
//     encrypted: Vec<u8>,
//     nonce: [u8; 16],
// }
//
// impl Logs {
//     pub fn img_hash_key(&self, mid: &MessageId, aid: &AttachmentId) -> ArrayString<[u8; 64]> {
//         blake3::hash(format!("{}.{}.{}", &self.nonce, mid, aid).as_bytes()).to_hex()
//     }
//
//     pub fn msg_hash_key(&self, mid: &MessageId) -> ArrayString<[u8; 64]> {
//         blake3::hash(format!("{}.{}", &self.nonce, mid).as_bytes()).to_hex()
//     }
//
//     pub fn msg_enc_key(&self, mid: &MessageId) -> [u8; 32] {
//         let mut key: [u8; 8] = [0u8; 8];
//         LittleEndian::write_u64(&mut key, mid.0);
//
//         blake3::hash(&key).into()
//     }
//
//     pub fn encrypt_msg(&self, m: &Message) -> Vec<u8> {
//         let mut msg_raw = serde_cbor::to_vec(m).unwrap();
//         let nonce = rand::random::<[u8; 16]>();
//         let mut enc = Salsa20::new_var(&self.msg_enc_key(&m.id), &nonce).unwrap();
//
//         enc.apply_keystream(msg_raw.as_mut_slice());
//         serde_cbor::to_vec(&EncryptedMessage {
//             encrypted: msg_raw,
//             nonce,
//         })
//         .unwrap()
//     }
//
//     pub fn decrypt_msg(&self, mut m: EncryptedMessage, mid: MessageId) -> Message {
//         let mut enc = Salsa20::new_var(&self.msg_enc_key(&mid), &m.nonce).unwrap();
//         enc.apply_keystream(m.encrypted.as_mut_slice());
//
//         serde_cbor::from_slice(m.encrypted.as_slice()).unwrap()
//     }
// }
