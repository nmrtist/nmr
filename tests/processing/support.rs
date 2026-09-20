use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::formats::bruker::{Parts, read_parts};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::{FourierExponentSign, FourierTransform, ProcessingOperation, ZeroFill};
use nmr::raw::{
    DirectSamples, IndirectComponents, LinearComponentTransform, PeriodicLaneModulation, RawAxis,
    RawAxisKind, RawDataset, RawDatasetBuilder, RawMetadata, ResolvedComponentTransform,
};
use std::f64::consts::PI;

pub(super) fn direct_axis(
    domain: AxisDomain,
    encoding: DirectSamples,
    points: usize,
    start: f64,
    step: f64,
) -> RawAxis {
    let (unit, coordinates) = match domain {
        AxisDomain::Time => (
            Some(AxisUnit::Second),
            AxisCoordinates::Uniform { start, step },
        ),
        AxisDomain::Frequency => (
            Some(AxisUnit::Hertz),
            AxisCoordinates::Uniform { start, step },
        ),
        _ => (None, AxisCoordinates::Unknown),
    };
    RawAxis::new(
        RawAxisKind::Direct(encoding),
        domain,
        unit,
        points,
        coordinates,
    )
    .unwrap()
}

pub(super) fn raw_dataset(axes: Vec<RawAxis>, samples: Vec<Complex64>) -> RawDataset {
    RawDatasetBuilder::new(axes, RawMetadata::default())
        .unwrap()
        .dense(samples)
        .unwrap()
}

pub(super) fn encoded_axis(
    points: usize,
    coefficients: Vec<Complex64>,
    modulation: PeriodicLaneModulation,
) -> RawAxis {
    let transform = LinearComponentTransform::try_new(2, coefficients, modulation).unwrap();
    RawAxis::new(
        RawAxisKind::Indirect(IndirectComponents::Encoded(
            ResolvedComponentTransform::user_constructed(transform).unwrap(),
        )),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        points,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.5,
        },
    )
    .unwrap()
}

pub(super) fn labeled_raw(label: &str) -> RawDataset {
    RawDatasetBuilder::new(
        vec![
            direct_axis(AxisDomain::Time, DirectSamples::Complex, 2, 0.0, 0.25)
                .with_label(Some(label.into())),
        ],
        RawMetadata::new(Some(format!("title {label}")), None, None, None, None).unwrap(),
    )
    .unwrap()
    .dense(vec![Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)])
    .unwrap()
}

pub(super) fn fft_operation(axis: usize, sign: FourierExponentSign) -> ProcessingOperation {
    ProcessingOperation::FourierTransform {
        axis,
        transform: FourierTransform::new(sign),
    }
}

pub(super) fn zero_fill_operation(axis: usize, points: usize) -> ProcessingOperation {
    ProcessingOperation::ZeroFill {
        axis,
        zero_fill: ZeroFill::new(points).unwrap(),
    }
}

pub(super) fn close(left: f64, right: f64) {
    assert!((left - right).abs() < 1e-9, "{left} != {right}");
}

pub(super) fn close_complex(left: Complex64, right: Complex64) {
    close(left.re, right.re);
    close(left.im, right.im);
}

pub(super) fn dft(input: &[Complex64], sign: FourierExponentSign) -> Vec<Complex64> {
    let sigma = if sign == FourierExponentSign::Negative {
        -1.0
    } else {
        1.0
    };
    (0..input.len())
        .map(|q| {
            input
                .iter()
                .enumerate()
                .map(|(n, value)| {
                    let angle = sigma * 2.0 * PI * n as f64 * q as f64 / input.len() as f64;
                    *value * Complex64::new(angle.cos(), angle.sin())
                })
                .sum()
        })
        .collect()
}

pub(super) fn imported_frequency_dataset() -> ProcessedDataset {
    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Hertz),
        4,
        AxisCoordinates::Uniform {
            start: -2.0,
            step: 1.0,
        },
        ComponentBasis::Cartesian,
    )
    .unwrap();
    let descriptor = ProcessedDescriptor::new(vec![axis]).unwrap();
    ProcessedDataset::new(
        descriptor,
        ProcessedData::new(vec![4], vec![2], vec![0.0; 8]).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Imported, vec![]).unwrap(),
    )
    .unwrap()
}

pub(super) fn bruker_raw(points: usize, grpdly: &str, extra: &str) -> RawDataset {
    let td = points * 2;
    let parameters = format!(
        "##TITLE= processing\n\
         ##$TD= {td}\n\
         ##$PARMODE= 0\n\
         ##$AQ_mod= 3\n\
         ##$BYTORDA= 0\n\
         ##$DTYPA= 0\n\
         ##$SW_h= 1000\n\
         ##$SFO1= 400\n\
         ##$BF1= 400\n\
         ##$GRPDLY= {grpdly}\n\
         {extra}\
         ##END=\n"
    );
    let values = (0..td as i32)
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    read_parts(Parts::new(&values, &[parameters.as_str()])).unwrap()
}

pub(super) fn bruker_oracle(mut values: Vec<Complex64>, delay: f64) -> Vec<Complex64> {
    let points = values.len();
    values.rotate_left(points / 2);
    let mut transformed = dft(&values, FourierExponentSign::Negative);
    for (index, value) in transformed.iter_mut().enumerate() {
        let angle = 2.0 * PI * delay * index as f64 / points as f64;
        *value = *value / points as f64 * Complex64::new(angle.cos(), angle.sin());
    }
    let mut output = dft(&transformed, FourierExponentSign::Positive);
    output.rotate_left(points.div_ceil(2));
    let skip = (delay + 2.0).floor() as usize;
    let fold = skip.saturating_sub(6);
    let tail = output[points - fold..].to_vec();
    for index in 0..fold {
        output[index] += tail[fold - 1 - index];
    }
    output.truncate(points - skip);
    output
}
