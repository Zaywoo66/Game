import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import { CubePage } from './map/CubePage';
import './style.css';

const smoke = new URLSearchParams(window.location.search).get('smoke') === '1';
const directMap = ['verify', 'verifyHost', 'testFrames', 'benchmark'].some((key) =>
  new URLSearchParams(window.location.search).get(key) === '1');

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>{smoke ? <CubePage /> : <App directMap={directMap} />}</React.StrictMode>,
);
