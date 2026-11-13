use std::f32::consts::PI;  // PI stała wbudowana
use std::f32::consts::TAU; // TAU to wbudowana stała równa 2 * PI

#[allow(dead_code)]
pub fn radiany_na_stopnie(x: f32) -> f32 {
    x * (180.0/PI)
}

#[allow(dead_code)]
pub fn stopnie_na_radiany(x: f32) -> f32 {
    x / (180.0/PI)
}

/// Przelicza współrzędne kartezjańskie (x, y) na biegunowe (r, beta),
/// gdzie beta jest w radianach w zakresie [0, 2*PI).
#[allow(dead_code)]
pub fn kartezjanskie_na_biegunowe(x: f32, y: f32) -> (f32, f32) {
    let r = x.hypot(y);      // R = sqrt(x^2 + y^2)
    let mut beta = y.atan2(x); // Zwraca zakres (-PI, PI]

    // Jeśli kąt jest ujemny (III lub IV ćwiartka), dodajemy 2*PI (TAU)
    if beta < 0.0 {
        beta += TAU;
    }

    (r, beta)
}

/// Przelicza współrzędne biegunowe (r, beta) na kartezjańskie (x, y).
/// Działa identycznie dla zakresu [0, 2*PI) jak i (-PI, PI].
#[allow(dead_code)]
pub fn biegunowe_na_kartezjanskie(r: f32, beta: f32) -> (f32, f32) {
    let (sin_beta, cos_beta) = beta.sin_cos();
    (r * cos_beta, r * sin_beta)
}

/// Przelicza współrzędne (x - wschód, y - północ) na (r, azymut)
/// Azymut w radianach z zakresu [0, 2*PI), gdzie 0 to Północ (Y+), a PI/2 to Wschód (X+).
pub fn kartezjanskie_na_azymut(x: f32, y: f32) -> (f32, f32) {
    let r = x.hypot(y);
    
    // Zamieniamy kolejność: x.atan2(y) zamiast y.atan2(x)!
    let mut azymut = x.atan2(y); 

    if azymut < 0.0 {
        azymut += TAU;
    }

    (r, azymut)
}

/// Przelicza (r, azymut) z powrotem na (x - wschód, y - północ)
pub fn azymut_na_kartezjanskie(r: f32, azymut: f32) -> (f32, f32) {
    let (sin_az, cos_az) = azymut.sin_cos();
    
    // Zamieniamy sinus i cosinus miejscami:
    let x = r * sin_az; // Wschód
    let y = r * cos_az; // Północ

    (x, y)
}

// Ta funkcja jest prywatna i zadziała tylko wewnątrz tego pliku
#[allow(dead_code)]
fn funkcja_pomocnicza() {
    println!("Pomagam!");
}

use eframe::egui;

pub const EARTH_RADIUS_KM: f32 = 6371.0;
pub const CENTER_LAT_DEG: f32 = 52.23529353245678; 
pub const CENTER_LON_DEG: f32 = 21.089756588924253;

// Konwersja (lat, lon) w stopniach -> (x, y) w km na kanwie (Azimuthal Equidistant)
#[allow(dead_code)]
pub fn geo_to_canvas(lat_deg: f32, lon_deg: f32) -> egui::Pos2 {
    let lat0 = CENTER_LAT_DEG.to_radians();
    //let lon0 = CENTER_LON_DEG.to_radians();
    let lat = lat_deg.to_radians();
    let d_lon = (lon_deg - CENTER_LON_DEG).to_radians();

    let cos_c = (lat0.sin() * lat.sin() + lat0.cos() * lat.cos() * d_lon.cos()).clamp(-1.0, 1.0);
    let c = cos_c.acos();

    let k = if c.abs() < 1e-6 { 1.0 } else { c / c.sin() };

    let x = EARTH_RADIUS_KM * k * lat.cos() * d_lon.sin();
    let y = EARTH_RADIUS_KM * k * (lat0.cos() * lat.sin() - lat0.sin() * lat.cos() * d_lon.cos());

    egui::pos2(x, y)
}

// Konwersja odwrotna (x, y) w km -> (lat, lon) w stopniach (dla kursora)
#[allow(dead_code)]
pub fn canvas_to_geo(x: f32, y: f32) -> (f32, f32) {
    let rho = (x * x + y * y).sqrt();
    if rho < 1e-5 {
        return (CENTER_LAT_DEG, CENTER_LON_DEG);
    }

    let lat0 = CENTER_LAT_DEG.to_radians();
    let lon0 = CENTER_LON_DEG.to_radians();
    let c = rho / EARTH_RADIUS_KM;

    let sin_c = c.sin();
    let cos_c = c.cos();

    let lat = (cos_c * lat0.sin() + (y * sin_c * lat0.cos()) / rho).asin();
    let lon = lon0 + (x * sin_c).atan2(rho * lat0.cos() * cos_c - y * lat0.sin() * sin_c);

    (lat.to_degrees(), lon.to_degrees())
}

