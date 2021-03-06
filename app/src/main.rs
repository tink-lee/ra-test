extern crate sgx_types;
extern crate sgx_urts;

use sgx_types::*;
use sgx_urts::SgxEnclave;

use std::os::unix::io::IntoRawFd;
use std::net::{TcpStream, SocketAddr};
use std::str;

static ENCLAVE_FILE: &'static str = "enclave.signed.so";
// const ENCLAVE_OUTPUT_BUF_MAX_LEN: usize = 32760 as usize;

extern {
    fn ecall_main(
        eid: sgx_enclave_id_t, retval: *mut sgx_status_t
    ) -> sgx_status_t;
}

pub fn lookup_ipv4(host: &str, port: u16) -> SocketAddr {
    use std::net::ToSocketAddrs;

    let addrs = (host, port).to_socket_addrs().unwrap();
    for addr in addrs {
        if let SocketAddr::V4(_) = addr {
            return addr;
        }
    }

    unreachable!("Cannot lookup address");
}

#[no_mangle]
pub extern "C"
fn ocall_sgx_init_quote(ret_ti: *mut sgx_target_info_t,
                        ret_gid : *mut sgx_epid_group_id_t) -> sgx_status_t {
    // println!("Entering ocall_sgx_init_quote");
    unsafe {sgx_init_quote(ret_ti, ret_gid)}
}

#[no_mangle]
pub extern "C"
fn ocall_get_ias_socket(ret_fd : *mut c_int) -> sgx_status_t {
    let port = 443;
    let hostname = "api.trustedservices.intel.com";
    let addr = lookup_ipv4(hostname, port);
    let sock = TcpStream::connect(&addr).expect("[-] Connect tls server failed!");

    unsafe {*ret_fd = sock.into_raw_fd();}

    sgx_status_t::SGX_SUCCESS
}

#[no_mangle]
pub extern "C"
fn ocall_get_quote (p_sigrl            : *const u8,
                    sigrl_len          : u32,
                    p_report           : *const sgx_report_t,
                    quote_type         : sgx_quote_sign_type_t,
                    p_spid             : *const sgx_spid_t,
                    p_nonce            : *const sgx_quote_nonce_t,
                    p_qe_report        : *mut sgx_report_t,
                    p_quote            : *mut u8,
                    _maxlen             : u32,
                    p_quote_len        : *mut u32) -> sgx_status_t {
    // println!("Entering ocall_get_quote");

    let mut real_quote_len : u32 = 0;

    let ret = unsafe {
        sgx_calc_quote_size(p_sigrl, sigrl_len, &mut real_quote_len as *mut u32)
    };

    if ret != sgx_status_t::SGX_SUCCESS {
        // println!("sgx_calc_quote_size returned {}", ret);
        return ret;
    }

    // println!("quote size = {}", real_quote_len);
    unsafe { *p_quote_len = real_quote_len; }

    let ret = unsafe {
        sgx_get_quote(p_report,
                      quote_type,
                      p_spid,
                      p_nonce,
                      p_sigrl,
                      sigrl_len,
                      p_qe_report,
                      p_quote as *mut sgx_quote_t,
                      real_quote_len)
    };

    if ret != sgx_status_t::SGX_SUCCESS {
        // println!("sgx_calc_quote_size returned {}", ret);
        return ret;
    }

    // println!("sgx_calc_quote_size returned {}", ret);
    ret
}

fn init_enclave() -> SgxResult<SgxEnclave> {
    let mut launch_token: sgx_launch_token_t = [0; 1024];
    let mut launch_token_updated: i32 = 0;
    // call sgx_create_enclave to initialize an enclave instance
    // Debug Support: set 2nd parameter to 1
    let debug = option_env!("SGX_DEBUG").unwrap_or("1");

    let mut misc_attr = sgx_misc_attribute_t {secs_attr: sgx_attributes_t {flags:0, xfrm:0}, misc_select:0};
    SgxEnclave::create(ENCLAVE_FILE,
                       if debug == "0" { 0 } else { 1 },
                       &mut launch_token,
                       &mut launch_token_updated,
                       &mut misc_attr)
}

fn main() {
    let enclave = match init_enclave() {
        Ok(r) => {
            r
        },
        Err(x) => {
            println!("[-] Init Enclave Failed {}!", x.as_str());
            return;
        },
    };

    let mut retval = sgx_status_t::SGX_SUCCESS;
    unsafe {
        ecall_main(enclave.geteid(), &mut retval);
    };
    match retval {
        sgx_status_t::SGX_SUCCESS => {},
        _ => {
            println!("[-] ECALL Enclave Failed {}!", retval.as_str());
            return;
        }
    }
    enclave.destroy();
}
