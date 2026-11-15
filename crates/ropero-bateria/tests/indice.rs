//! El índice de la batería: se parsea entero o dice **qué línea** está mal.
//!
//! El índice (`docs/tests/indice.md`) es la rúbrica: declara 16 tests en 6
//! grupos, el peso de cada test, el total por grupo y el perfil de
//! dimensiones. Un índice que no cuadra **no** se interpreta: se dice la
//! línea y qué falta. Aquí no se inventa ningún peso.

use ropero_bateria::indice::parse_indice;

/// Un índice mínimo, con la forma exacta del real.
const INDICE_MINIMO: &str = r#"# Índice de la batería

## Grupo A — Razonamiento lógico-deductivo

### A1 · `test-logico-deductivo.md` — Deducción en cadena

**Peso sugerido:** 5
**Objetivo:** encadenar premisas.

## Grupo B — Razonamiento cuantitativo y probabilístico

### B1 · `test-calibracion.md` — Confianza por pregunta

**Peso sugerido:** 4
**Objetivo:** estimar con probabilidad.

## Resumen de pesos por grupo

| Grupo | Tests | Peso total sugerido |
|---|---|---|
| A · Lógico-deductivo | 1 | 5 |
| B · Cuantitativo | 1 | 4 |
| **Total** | **2** | **9** |

## Perfil de salida recomendado

| Dimensión | Tests que la miden |
|---|---|
| Deducción formal | A1 |
| Abstracción/patrones | A1 |
| Calibración/verbalización | B1 |
"#;

/// La línea (1-based) en la que empieza algo del índice.
fn linea(texto: &str, principio: &str) -> usize {
    texto
        .lines()
        .position(|l| l.starts_with(principio))
        .map(|n| n + 1)
        .expect("el índice tiene esa línea")
}

/// El índice real del repo: la rúbrica que se cumple de verdad.
fn indice_real() -> String {
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/tests/indice.md");
    std::fs::read_to_string(&ruta)
        .expect("docs/tests/indice.md tiene que existir para poder puntuar")
}

#[test]
fn el_indice_real_se_parsea_entero() {
    let bateria = parse_indice(&indice_real()).expect("el índice real se parsea");
    assert_eq!(bateria.tests.len(), 16, "la batería tiene 16 tests");
    assert_eq!(bateria.grupos.len(), 6, "la batería tiene 6 grupos");
    assert_eq!(
        bateria.dimensiones.len(),
        12,
        "la batería tiene 12 dimensiones"
    );
    assert!(
        bateria.avisos.is_empty(),
        "el índice real no tiene avisos: {:?}",
        bateria.avisos
    );
}

#[test]
fn cada_test_declara_su_id_su_grupo_su_fichero_su_titulo_y_su_objetivo() {
    let bateria = parse_indice(&indice_real()).expect("el índice real se parsea");
    let a1 = bateria.test("A1").expect("el test A1 está");
    assert_eq!(a1.grupo, 'A');
    assert_eq!(a1.fichero, "test-logico-deductivo.md");
    assert_eq!(a1.titulo, "Puzle de casas (tipo Einstein)");
    assert!(!a1.objetivo.is_empty(), "A1 declara objetivo");
}

#[test]
fn los_pesos_de_cada_test_son_los_del_indice() {
    let bateria = parse_indice(&indice_real()).expect("el índice real se parsea");
    let esperado: [(&str, u32); 16] = [
        ("A1", 5),
        ("A2", 4),
        ("A3", 5),
        ("A4", 3),
        ("A5", 5),
        ("B1", 5),
        ("B2", 5),
        ("C1", 4),
        ("C2", 4),
        ("D1", 5),
        ("D2", 5),
        ("D3", 5),
        ("E1", 4),
        ("E2", 4),
        ("F1", 5),
        ("F2", 3),
    ];
    for (id, peso) in esperado {
        assert_eq!(
            bateria.test(id).expect("el test está").peso,
            peso,
            "el peso de {id}"
        );
    }
}

#[test]
fn los_totales_de_grupo_cuadran_con_la_tabla() {
    let bateria = parse_indice(&indice_real()).expect("el índice real se parsea");
    let esperado: [(char, usize, u32); 6] = [
        ('A', 5, 22),
        ('B', 2, 10),
        ('C', 2, 8),
        ('D', 3, 15),
        ('E', 2, 8),
        ('F', 2, 8),
    ];
    let mut total = 0u32;
    for (letra, tests, peso) in esperado {
        let grupo = bateria.grupo(letra).expect("el grupo está");
        assert_eq!(
            grupo.ids.len(),
            tests,
            "el grupo {letra} tiene {tests} tests"
        );
        assert_eq!(grupo.peso_total, peso, "el grupo {letra} pesa {peso}");
        assert_eq!(
            bateria
                .tests_del_grupo(letra)
                .iter()
                .map(|t| t.peso)
                .sum::<u32>(),
            peso,
            "los pesos de {letra} suman {peso}"
        );
        total += peso;
    }
    assert_eq!(total, 71, "la batería pesa 71");
}

#[test]
fn los_grupos_se_nombran_como_el_indice() {
    let bateria = parse_indice(&indice_real()).expect("el índice real se parsea");
    assert_eq!(
        bateria.grupo('A').expect("A").nombre,
        "Razonamiento lógico-deductivo"
    );
    assert_eq!(
        bateria.grupo('D').expect("D").nombre,
        "Robustez, honestidad y auto-conciencia"
    );
}

#[test]
fn las_dimensiones_alimentan_los_tests_que_declara_la_tabla() {
    let bateria = parse_indice(&indice_real()).expect("el índice real se parsea");
    assert_eq!(
        bateria.dimension("Abstracción/patrones").expect("está").ids,
        ["A2"]
    );
    assert_eq!(
        bateria
            .dimension("Honestidad ante incertidumbre")
            .expect("está")
            .ids,
        ["C2", "D2"]
    );
    assert_eq!(
        bateria
            .dimension("Control de instrucciones")
            .expect("está")
            .ids,
        ["F1", "F2"]
    );
}

#[test]
fn un_indice_minimo_se_parsea() {
    let bateria = parse_indice(INDICE_MINIMO).expect("el índice mínimo se parsea");
    assert_eq!(bateria.tests.len(), 2);
    assert_eq!(bateria.grupos.len(), 2);
    assert_eq!(bateria.dimensiones.len(), 3);
}

// ─────────────────────────────────────────────────────────────────────────────
// Lo que no cuadra: error con la línea y con el valor recibido
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn un_test_sin_peso_declarado_es_error_que_nombra_el_test_y_la_linea() {
    let texto = INDICE_MINIMO.replace("**Peso sugerido:** 4", "4");
    let error = parse_indice(&texto).expect_err("sin peso no se puede puntuar");
    assert_eq!(
        error.linea,
        linea(&texto, "### B1"),
        "la línea del test sin peso"
    );
    assert!(error.mensaje.contains("B1"), "nombra el test: {error}");
    assert!(
        error.mensaje.contains("Peso sugerido"),
        "dice qué añadir: {error}"
    );
}

#[test]
fn un_peso_que_no_cuadra_con_la_tabla_es_error() {
    let texto = INDICE_MINIMO.replace("**Peso sugerido:** 4", "**Peso sugerido:** 3");
    let error = parse_indice(&texto).expect_err("la tabla dice 4 y los tests suman 3");
    assert_eq!(
        error.linea,
        linea(&texto, "| B ·"),
        "la línea de la fila de grupos"
    );
    assert!(error.mensaje.contains("B"), "nombra el grupo: {error}");
    assert!(
        error.mensaje.contains("3"),
        "dice lo que suman los tests: {error}"
    );
    assert!(
        error.mensaje.contains("4"),
        "dice lo que dice la tabla: {error}"
    );
}

#[test]
fn el_recuento_de_tests_del_grupo_no_cuadra_es_error() {
    let texto = INDICE_MINIMO.replace(
        "| A · Lógico-deductivo | 1 | 5 |",
        "| A · Lógico-deductivo | 2 | 5 |",
    );
    let error = parse_indice(&texto).expect_err("la tabla dice 2 tests y hay 1");
    assert_eq!(
        error.linea,
        linea(&texto, "| A ·"),
        "la línea de la fila de grupos"
    );
    assert!(
        error.mensaje.contains("2"),
        "dice lo que dice la tabla: {error}"
    );
    assert!(error.mensaje.contains("1"), "dice lo que hay: {error}");
}

#[test]
fn el_total_de_la_bateria_no_cuadra_es_error() {
    let texto = INDICE_MINIMO.replace(
        "| **Total** | **2** | **9** |",
        "| **Total** | **2** | **8** |",
    );
    let error = parse_indice(&texto).expect_err("el total de la tabla no es 9");
    assert_eq!(
        error.linea,
        linea(&texto, "| **Total**"),
        "la línea de la fila de total"
    );
    assert!(
        error.mensaje.contains("8"),
        "dice lo que dice la tabla: {error}"
    );
    assert!(
        error.mensaje.contains("9"),
        "dice lo que suman los tests: {error}"
    );
}

#[test]
fn un_test_debe_estar_dentro_de_un_grupo() {
    let texto = INDICE_MINIMO.replace("# Índice de la batería", "### Z1 · `x.md` — Fuera");
    let error = parse_indice(&texto).expect_err("un test suelto no se puede puntuar");
    assert_eq!(
        error.linea,
        linea(&texto, "### Z1"),
        "la línea del test suelto"
    );
    assert!(
        error.mensaje.contains("grupo"),
        "dice que falta el grupo: {error}"
    );
}

#[test]
fn un_test_debe_pertenecer_al_grupo_en_el_que_esta() {
    let texto = INDICE_MINIMO.replace("### A1 ·", "### B2 ·");
    let error = parse_indice(&texto).expect_err("B2 no está bajo el grupo A");
    assert_eq!(error.linea, linea(&texto, "### B2 ·"), "la línea del test");
    assert!(error.mensaje.contains("B2"), "nombra el test: {error}");
    assert!(
        error.mensaje.contains("A"),
        "nombra el grupo en el que está: {error}"
    );
}

#[test]
fn un_encabezado_de_test_malformado_es_error_con_la_linea() {
    let texto = INDICE_MINIMO.replace(
        "### A1 · `test-logico-deductivo.md` — Deducción en cadena",
        "### A1 test-logico-deductivo.md Deducción",
    );
    let error = parse_indice(&texto).expect_err("el encabezado no tiene la forma del índice");
    assert_eq!(
        error.linea,
        linea(&texto, "### A1 test"),
        "la línea malformada"
    );
    assert!(
        error.mensaje.contains("formato"),
        "dice el formato del índice: {error}"
    );
}

#[test]
fn un_id_repetido_es_error() {
    let texto = INDICE_MINIMO.replace("### B1 ·", "### A1 ·");
    let error = parse_indice(&texto).expect_err("el id A1 está repetido");
    assert!(
        error.mensaje.contains("A1"),
        "nombra el id repetido: {error}"
    );
    assert!(
        error.mensaje.contains("repetido"),
        "dice que está repetido: {error}"
    );
}

#[test]
fn una_dimension_que_menta_un_test_inexistente_es_error() {
    let texto = INDICE_MINIMO.replace("| Deducción formal | A1 |", "| Deducción formal | A1, Z9 |");
    let error = parse_indice(&texto).expect_err("Z9 no es un test de la batería");
    assert_eq!(
        error.linea,
        linea(&texto, "| Deducción formal |"),
        "la línea de la dimensión"
    );
    assert!(
        error.mensaje.contains("Z9"),
        "nombra el id desconocido: {error}"
    );
}

#[test]
fn una_dimension_sin_tests_es_error() {
    let texto = INDICE_MINIMO.replace(
        "| Abstracción/patrones | A1 |",
        "| Abstracción/patrones |  |",
    );
    let error = parse_indice(&texto).expect_err("una dimensión sin tests no alimenta nada");
    assert!(
        error.mensaje.contains("Abstracción/patrones"),
        "nombra la dimensión: {error}"
    );
}

#[test]
fn un_test_que_no_alimenta_ninguna_dimension_deja_aviso_no_error() {
    let texto = INDICE_MINIMO.replace("| Calibración/verbalización | B1 |", "");
    let bateria = parse_indice(&texto).expect("el índice sigue siendo una batería");
    assert_eq!(
        bateria.tests.len(),
        2,
        "la batería sigue teniendo sus tests"
    );
    assert_eq!(
        bateria.avisos,
        ["aviso: el test 'B1' no alimenta ninguna dimensión (perfil incompleto)"],
        "el aviso dice qué test queda fuera del perfil"
    );
}

#[test]
fn un_indice_vacio_es_error() {
    let error = parse_indice("").expect_err("un índice vacío no es una batería");
    assert_eq!(error.linea, 1);
    assert!(
        error.mensaje.contains("tests"),
        "dice que no hay tests: {error}"
    );
}

#[test]
fn un_grupo_representado_dos_veces_es_error() {
    let texto = INDICE_MINIMO.replace(
        "## Grupo B — Razonamiento cuantitativo y probabilístico",
        "## Grupo A — Razonamiento cuantitativo y probabilístico",
    );
    let error = parse_indice(&texto).expect_err("el grupo A aparece dos veces");
    assert_eq!(
        error.linea,
        linea(&texto, "## Grupo A — Razonamiento cuantitativo"),
        "la línea del grupo repetido"
    );
    assert!(error.mensaje.contains("A"), "nombra el grupo: {error}");
}

#[test]
fn un_peso_cero_es_error() {
    let texto = INDICE_MINIMO.replace("**Peso sugerido:** 5", "**Peso sugerido:** 0");
    let error = parse_indice(&texto).expect_err("un peso de 0 no puntuaría nada");
    assert!(
        error.mensaje.contains("0"),
        "dice el valor recibido: {error}"
    );
}

#[test]
fn un_peso_no_numericos_es_error() {
    let texto = INDICE_MINIMO.replace("**Peso sugerido:** 5", "**Peso sugerido:** cinco");
    let error = parse_indice(&texto).expect_err("el peso no es un número");
    assert!(
        error.mensaje.contains("cinco"),
        "dice el valor recibido: {error}"
    );
}

#[test]
fn la_tabla_de_grupos_es_obligatoria() {
    let texto = INDICE_MINIMO.replace("## Resumen de pesos por grupo", "## Resumen");
    let error = parse_indice(&texto).expect_err("sin tabla de grupos no se valida el peso");
    assert!(
        error.mensaje.contains("Resumen de pesos por grupo"),
        "dice qué sección falta: {error}"
    );
}

#[test]
fn el_perfil_de_dimensiones_es_obligatorio() {
    let texto = INDICE_MINIMO.replace("## Perfil de salida recomendado", "## Perfil");
    let error = parse_indice(&texto).expect_err("sin perfil no hay dimensiones");
    assert!(
        error.mensaje.contains("Perfil de salida recomendado"),
        "dice qué sección falta: {error}"
    );
}
