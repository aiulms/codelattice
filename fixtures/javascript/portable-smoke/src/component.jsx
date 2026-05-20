import React from 'react';

export function Button({ children, onClick, variant = 'primary' }) {
    const handleClick = () => {
        if (onClick) {
            onClick();
        }
    };

    return (
        <button className={`btn btn-${variant}`} onClick={handleClick}>
            {children}
        </button>
    );
}

export class Card extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isExpanded: false
        };
    }

    toggleExpanded = () => {
        this.setState(prev => ({
            isExpanded: !prev.isExpanded
        }));
    };

    render() {
        const { title, children } = this.props;
        const { isExpanded } = this.state;

        return (
            <div className="card">
                <div className="card-header" onClick={this.toggleExpanded}>
                    <h3>{title}</h3>
                    <span>{isExpanded ? '▼' : '▶'}</span>
                </div>
                {isExpanded && <div className="card-body">{children}</div>}
            </div>
        );
    }
}

export default function App() {
    return (
        <div className="app">
            <h1>Hello CodeLattice</h1>
            <Button onClick={() => console.log('clicked')}>
                Click me
            </Button>
        </div>
    );
}
