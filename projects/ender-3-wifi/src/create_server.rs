use embedded_svc::wifi::{self, AuthMethod};
use esp_idf_hal::modem::Modem;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::http::server::EspHttpServer;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};

const SSID: &str = env!("WIFI_SSID");
const PASSWORD: &str = env!("WIFI_PASS");
const STACK_SIZE: usize = 10000;

pub fn create_server(modem: &mut Modem) -> EspHttpServer {
    let sys_loop = EspSystemEventLoop::take().expect("Should give system event loop");
    let nvs = EspDefaultNvsPartition::take().expect("Should give esp nvs partition");

    let esp_wifi =
        EspWifi::new(modem, sys_loop.clone(), Some(nvs)).expect("Should create esp wifi");
    let mut wifi =
        BlockingWifi::wrap(esp_wifi, sys_loop).expect("Should create blocking wifi wrapper");

    let wifi_configuration = wifi::Configuration::Client(wifi::ClientConfiguration {
        ssid: SSID.into(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: PASSWORD.into(),
        channel: None,
    });

    wifi.set_configuration(&wifi_configuration)
        .expect("Should configure wifi");
    wifi.start().expect("Should start wifi");
    wifi.connect().expect("Should connect wifi");
    wifi.wait_netif_up().expect("Should wait for netif up");

    let server_configuration = esp_idf_svc::http::server::Configuration {
        stack_size: STACK_SIZE,
        ..Default::default()
    };

    core::mem::forget(wifi);

    EspHttpServer::new(&server_configuration).expect("Should create esp http server")
}
