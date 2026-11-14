use std::io::{self, Read, Write};

// Frame: 1 byte type, 4 byte big-endian length, payload.
// client->daemon: C (json command). daemon->client: J (json reply).
// shim<->client (relayed by the daemon after `attach`): H hello json, O output, I input,
// R resize (rows u16, cols u16), E close stdin, X exit json.
pub fn write_frame(w: &mut impl Write, t: u8, p: &[u8]) -> io::Result<()> {
    let mut b = Vec::with_capacity(5 + p.len());
    b.push(t);
    b.extend_from_slice(&(p.len() as u32).to_be_bytes());
    b.extend_from_slice(p);
    w.write_all(&b)?;
    w.flush()
}

pub fn read_frame(r: &mut impl Read) -> io::Result<(u8, Vec<u8>)> {
    let mut h = [0u8; 5];
    r.read_exact(&mut h)?;
    let n = u32::from_be_bytes([h[1], h[2], h[3], h[4]]) as usize;
    if n > 64 << 20 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    let mut p = vec![0u8; n];
    r.read_exact(&mut p)?;
    Ok((h[0], p))
}
