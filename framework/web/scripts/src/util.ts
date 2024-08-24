export const failUnless = (condition: boolean) => {
  if (!condition) {
    process.exit(1);
  }
};
