import { useTheme } from "next-themes";

type SvgProps = { alt: string } & (
  | {
      src: string;
    }
  | {
      lightSrc: string;
      darkSrc: string;
    }
);

const Svg = (props: SvgProps) => {
  const { alt } = props;
  const { lightSrc, darkSrc } = (() => {
    if ("src" in props) {
      return {
        lightSrc: props.src,
        darkSrc: props.src,
      };
    } else {
      return {
        lightSrc: props.lightSrc,
        darkSrc: props.darkSrc,
      };
    }
  })();
  const { theme } = useTheme();
  if (theme === "dark") {
    return <img src={"/conformal/" + darkSrc} alt={alt} />;
  } else if (theme === "light") {
    return <img src={"/conformal/" + lightSrc} alt={alt} />;
  }
  return (
    <picture>
      <source
        srcSet={"/conformal/" + darkSrc}
        media="(prefers-color-scheme: dark)"
      />
      <source
        srcSet={"/conformal/" + lightSrc}
        media="(prefers-color-scheme: light)"
      />
      <img src={"/conformal/" + darkSrc} alt={alt} />
    </picture>
  );
};

export default Svg;
