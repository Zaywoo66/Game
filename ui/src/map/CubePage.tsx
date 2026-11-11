import { useEffect, useRef, useState } from 'react';
import * as THREE from 'three';

export function CubePage() {
  const mount = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState('Initializing WebGL…');

  useEffect(() => {
    const host = mount.current;
    if (!host) return;
    let renderer: THREE.WebGLRenderer;
    try {
      renderer = new THREE.WebGLRenderer({ antialias: true });
    } catch (error) {
      console.error('WebGL initialization failed:', error);
      setStatus('WebGL initialization failed. Check the developer console.');
      return;
    }

    const gl = renderer.getContext();
    const debug = gl.getExtension('WEBGL_debug_renderer_info');
    const gpu = debug
      ? gl.getParameter(debug.UNMASKED_RENDERER_WEBGL)
      : gl.getParameter(gl.RENDERER);
    const version = gl.getParameter(gl.VERSION);
    console.info(`WebGL renderer/GPU: ${gpu}; version: ${version}`);
    setStatus(`WebGL renderer/GPU: ${gpu}`);

    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    host.appendChild(renderer.domElement);
    const scene = new THREE.Scene();
    scene.background = new THREE.Color('#101728');
    const camera = new THREE.PerspectiveCamera(60, 1, 0.1, 100);
    camera.position.z = 3;
    const geometry = new THREE.BoxGeometry();
    const material = new THREE.MeshNormalMaterial();
    const cube = new THREE.Mesh(geometry, material);
    scene.add(cube);

    const resize = () => {
      const width = host.clientWidth;
      const height = host.clientHeight;
      if (width === 0 || height === 0) return;
      renderer.setSize(width, height);
      camera.aspect = width / height;
      camera.updateProjectionMatrix();
    };
    const observer = new ResizeObserver(resize);
    observer.observe(host);
    resize();
    let frame = 0;
    const animate = () => {
      frame = requestAnimationFrame(animate);
      cube.rotation.x += 0.01;
      cube.rotation.y += 0.015;
      renderer.render(scene, camera);
    };
    animate();
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      geometry.dispose();
      material.dispose();
      renderer.dispose();
      host.removeChild(renderer.domElement);
    };
  }, []);

  return <main><h1>Strategy Foundation</h1><p>{status}</p><div className="viewport" ref={mount} /></main>;
}

