/*

MIT License

DELABELLA - Delaunay triangulation library
Copyright (C) 2018 GUMIX - Marcin Sokalski

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

*/

// The shim's copy of src/BRepMesh/delabella.pxx of OCCT 7.9.3, changed as ADR 0039 lists: in the
// shim's namespace, without the deprecated function and the C interface, comments on their own
// lines.

#ifndef DELABELLA_H
#define DELABELLA_H

namespace partsmith
{
namespace occt
{
namespace delabella
{

struct DelaBella_Vertex
{
  // index of original point
  int               i;
  // coordinates (input copy)
  double            x, y;
  // next silhouette vertex
  DelaBella_Vertex* next;
};

struct DelaBella_Triangle
{
  // 3 vertices spanning this triangle
  DelaBella_Vertex*   v[3];
  // 3 adjacent faces, f[i] is at the edge opposite to vertex v[i]
  DelaBella_Triangle* f[3];
  // next triangle (of delaunay set or hull set)
  DelaBella_Triangle* next;
};

struct IDelaBella
{
  static IDelaBella* Create();

  virtual void Destroy() = 0;

  virtual void SetErrLog(int (*proc)(void* stream, const char* fmt, ...), void* stream) = 0;

  // return 0: no output
  // negative: all points are colinear, output hull vertices form colinear segment list, no
  // triangles on output positive: output hull vertices form counter-clockwise ordered segment
  // contour, delaunay and hull triangles are available if 'y' pointer is null, y coords are treated
  // to be located immediately after every x if advance_bytes is less than 2*sizeof coordinate type,
  // it is treated as 2*sizeof coordinate type
  virtual int Triangulate(int          points,
                          const float* x,
                          const float* y             = 0,
                          int          advance_bytes = 0) = 0;
  virtual int Triangulate(int           points,
                          const double* x,
                          const double* y             = 0,
                          int           advance_bytes = 0) = 0;

  // num of points passed to last call to Triangulate()
  virtual int GetNumInputPoints() const = 0;

  // num of verts returned from last call to Triangulate()
  virtual int GetNumOutputVerts() const = 0;

  // valid only if Triangulate() > 0
  virtual const DelaBella_Triangle* GetFirstDelaunayTriangle() const = 0;
  // valid only if Triangulate() > 0
  virtual const DelaBella_Triangle* GetFirstHullTriangle() const = 0;
  // if Triangulate() < 0 it is list, otherwise closed contour!
  virtual const DelaBella_Vertex* GetFirstHullVertex() const = 0;
};

} // namespace delabella
} // namespace occt
} // namespace partsmith

#endif
