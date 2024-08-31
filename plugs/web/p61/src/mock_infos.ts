import { Info } from "@conformal/plugin";

const infos = new Map<string, Info>(
  Object.entries({
    dco1_shape: {
      title: "DCO1 Shape",
      type_specific: {
        t: "enum",
        default: "Saw",
        values: ["Saw", "Pulse", "PWM"],
      },
    },
    dco1_width: {
      title: "DCO1 Width",
      type_specific: {
        t: "numeric",
        default: 50.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
    dco1_octave: {
      title: "DCO1 Octave",
      type_specific: {
        t: "enum",
        default: "Med",
        values: ["Low", "Med", "High"],
      },
    },
    dco2_shape: {
      title: "DCO2 Shape",
      type_specific: {
        t: "enum",
        default: "Off",
        values: ["Off", "Saw", "Square"],
      },
    },
    dco2_octave: {
      title: "DCO2 Octave",
      type_specific: {
        t: "enum",
        default: "Med",
        values: ["Low", "Med", "High"],
      },
    },
    dco2_detune: {
      title: "DCO2 Detune",
      type_specific: {
        t: "numeric",
        default: 0.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
    dco2_interval: {
      title: "DCO2 Interval",
      type_specific: {
        t: "enum",
        default: "1",
        values: ["-3", "1", "3", "4", "5"],
      },
    },
    attack: {
      title: "Attack Time",
      type_specific: {
        t: "numeric",
        default: 0.001,
        valid_range: [0.001, 10.0],
        units: "s",
      },
    },
    decay: {
      title: "Decay Time",
      type_specific: {
        t: "numeric",
        default: 0.001,
        valid_range: [0.001, 10.0],
        units: "s",
      },
    },
    sustain: {
      title: "Sustain",
      type_specific: {
        t: "numeric",
        default: 100.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
    release: {
      title: "Release Time",
      type_specific: {
        t: "numeric",
        default: 0.001,
        valid_range: [0.001, 10.0],
        units: "s",
      },
    },
    mg_rate: {
      title: "MG Rate",
      type_specific: {
        t: "numeric",
        default: 60.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
    mg_delay: {
      title: "MG Delay",
      type_specific: {
        t: "numeric",
        default: 0.0,
        valid_range: [0.0, 10.0],
        units: "s",
      },
    },
    mg_pitch: {
      title: "MG Pitch",
      type_specific: {
        t: "numeric",
        default: 0.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
    mg_vcf: {
      title: "MG VCF",
      type_specific: {
        t: "numeric",
        default: 0.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
    vcf_cutoff: {
      title: "VCF Cutoff",
      type_specific: {
        t: "numeric",
        default: 64.0,
        valid_range: [0.0, 128.0],
        units: "",
      },
    },
    vcf_resonance: {
      title: "VCF Resonance",
      type_specific: {
        t: "numeric",
        default: 0.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
    vcf_tracking: {
      title: "VCF Tracking",
      type_specific: {
        t: "numeric",
        default: 66.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
    vcf_env: {
      title: "VCF Env",
      type_specific: {
        t: "numeric",
        default: 0.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
    vcf_velocity: {
      title: "VCF Velocity",
      type_specific: {
        t: "numeric",
        default: 0.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
    vca_mode: {
      title: "VCA Mode",
      type_specific: {
        t: "enum",
        default: "Envelope",
        values: ["Gate", "Envelope"],
      },
    },
    vca_level: {
      title: "VCA Level",
      type_specific: {
        t: "numeric",
        default: 80.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
    vca_velocity: {
      title: "VCA Velocity",
      type_specific: {
        t: "numeric",
        default: 0.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
    wheel_rate: {
      title: "Wheel Rate",
      type_specific: {
        t: "numeric",
        default: 0.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
    wheel_dco: {
      title: "Wheel DCO Depth",
      type_specific: {
        t: "numeric",
        default: 0.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
    wheel_vcf: {
      title: "Wheel VCF Depth",
      type_specific: {
        t: "numeric",
        default: 0.0,
        valid_range: [0.0, 100.0],
        units: "%",
      },
    },
  }),
);

export default infos;
