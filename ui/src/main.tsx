import React from 'react';
import ReactDOM from 'react-dom/client';
import { CubePage } from './map/CubePage';
import './style.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode><CubePage /></React.StrictMode>,
);

