/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    fontSize: {
      sm: "0.625rem",
      base: "1rem",
      logo: "2.5rem",
    },
    fontFamily: {
      sans: ["Source Sans Pro"],
    },
    colors: {
      backg: "#0F1A20",
      zone: "#25283D",
      border: "#6290C8",
      pop: "#F26DF9",
      pop2: "#FCF7F8",
    },
  },
  plugins: [],
};
