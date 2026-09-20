use nmr::processing::{
    DelaySource, DigitalFilterCorrection, FourierTransform, ProcessingOperation, ProcessingPlan,
};

pub const TABLE_PARAMETERS: &str = "##$GRPDLY= -1\n##$DSPFVS= 12\n##$DECIM= 16\n";

pub fn write_input(directory: &std::path::Path, delay_parameters: &str) -> std::io::Result<()> {
    let parameters = format!(
        "##TITLE= synthetic DSP regression\n##$TD= 256\n##$PARMODE= 0\n\
         ##$AQ_mod= 3\n##$BYTORDA= 0\n##$DTYPA= 0\n##$SW_h= 8000\n\
         ##$SFO1= 400.13\n##$BF1= 400\n##$O1= 1880\n{delay_parameters}##END=\n"
    );
    std::fs::write(directory.join("acqus"), parameters)?;
    let bytes = (0i32..256)
        .flat_map(|i| ((i * 17) % 113 - 56).to_le_bytes())
        .collect::<Vec<_>>();
    std::fs::write(directory.join("fid"), bytes)
}

pub fn fft() -> ProcessingPlan {
    ProcessingPlan::new(vec![ProcessingOperation::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    }])
    .unwrap()
}

pub fn correction() -> ProcessingPlan {
    ProcessingPlan::new(vec![ProcessingOperation::DigitalFilterCorrection {
        axis: 0,
        correction: DigitalFilterCorrection::FrequencyDomainPhaseRampV1(DelaySource::AxisEvidence),
    }])
    .unwrap()
}
