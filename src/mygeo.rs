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
pub fn biegunowe_na_kartezjanskie(r: f32, beta: f32) -> (f32, f32) {
    let (sin_beta, cos_beta) = beta.sin_cos();
    (r * cos_beta, r * sin_beta)
}

// Ta funkcja jest prywatna i zadziała tylko wewnątrz tego pliku
#[allow(dead_code)]
fn funkcja_pomocnicza() {
    println!("Pomagam!");
}


