import React, { ReactNode } from "react";

export interface Props {
  children?: ReactNode;
}

interface State {
  hasError: boolean;
}

class IgnoreErrorBoundary extends React.Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  render() {
    if (this.state.hasError) {
      return <></>;
    }

    return this.props.children;
  }
}
export default IgnoreErrorBoundary;
