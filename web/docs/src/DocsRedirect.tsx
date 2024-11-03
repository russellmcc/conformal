import { useRouter } from "next/router";
import { useEffect } from "react";

const Page = () => {
  const router = useRouter();
  useEffect(() => {
    void router.push("/docs");
  });
  return <></>;
};

export default Page;
