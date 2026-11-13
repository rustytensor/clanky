//    ██
// TSX fixture for the Q3 syntax gate.
//                        ██
type Props = { title: string };
export function Title(props: Props) {
  const size = 3;
  return <h1 className="t">{props.title}</h1>;
}
