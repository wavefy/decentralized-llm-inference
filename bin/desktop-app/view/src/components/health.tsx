import { registryUrl } from "@/lib/utils";
import { useSwarmHealth } from "@/queries/health";

const Health = () => {
  const { data, isLoading } = useSwarmHealth({
    registryUrl,
  });

  return (<>HelloWorld</>)

};

export default Health;
